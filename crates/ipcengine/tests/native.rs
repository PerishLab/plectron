use ipcengine::{
    message::Connection,
    native::{Phase, Publisher, Receiver},
};
use std::io::Write;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
struct Process {
    root: PathBuf,
    child: Child,
    control: Connection,
    publisher: Option<Publisher>,
}
impl Process {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "ipc-native-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        ipcengine::endpoint::prepare(&root).unwrap();
        let listener = UnixListener::bind(root.join("data")).unwrap();
        let control = UnixListener::bind(root.join("control")).unwrap();
        let child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "child", "--nocapture"])
            .env("PLECTRON_IPC_CHILD", &root)
            .stdout(Stdio::null())
            .spawn()
            .unwrap();
        let socket = accept(&listener);
        let control = Connection::new(accept(&control)).unwrap();
        let publisher = Publisher::new(socket, &root, 65_536).unwrap();
        Self {
            root,
            child,
            control,
            publisher: Some(publisher),
        }
    }
    fn ready(&mut self, generation: u64) {
        let report: (u64, u64) = self.control.read(1024, Duration::from_secs(6)).unwrap();
        assert_eq!(report, (generation, generation * 65_536));
    }
    fn command(&mut self, command: &str) {
        self.control.write(&command, 1024).unwrap();
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn accept(listener: &UnixListener) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        match listener.accept() {
            Ok((socket, _)) => return socket,
            Err(error) => assert_eq!(error.kind(), std::io::ErrorKind::WouldBlock),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(2));
    }
}
#[test]
fn child() {
    let Some(root) = std::env::var_os("PLECTRON_IPC_CHILD") else {
        return;
    };
    let root = PathBuf::from(root);
    let socket = UnixStream::connect(root.join("data")).unwrap();
    let disrupt = socket.try_clone().unwrap();
    let mut receiver = Receiver::new(socket).unwrap();
    let mut control = Connection::connect(&root.join("control")).unwrap();
    loop {
        let lease = receiver.acquire().unwrap();
        let mut bytes = vec![0; lease.length()];
        lease.copy(&mut bytes).unwrap();
        assert!(
            bytes
                .iter()
                .all(|byte| u64::from(*byte) == lease.generation())
        );
        control
            .write(&(lease.generation(), lease.checksum()), 1024)
            .unwrap();
        let command: String = control.read(1024, Duration::from_secs(6)).unwrap();
        match command.as_str() {
            "return" => lease.finish().unwrap(),
            "exit" => std::process::exit(0),
            "disconnect" => {
                disrupt.shutdown(std::net::Shutdown::Both).unwrap();
                let _: String = control.read(1024, Duration::from_secs(6)).unwrap();
                control
                    .write(&(lease.generation(), lease.checksum()), 1024)
                    .unwrap();
                return;
            }
            "survive" => {
                control
                    .write(&(lease.generation(), lease.checksum()), 1024)
                    .unwrap();
                assert!(lease.finish().is_err());
                return;
            }
            "finish" => {
                lease.finish().unwrap();
                return;
            }
            _ => panic!("unknown command"),
        }
    }
}
#[test]
fn generations() {
    let mut process = Process::start();
    let monitor = process.publisher.as_ref().unwrap().monitor();
    for generation in 1..=3 {
        let publisher = process.publisher.as_mut().unwrap();
        publisher.fill(generation).unwrap();
        assert_eq!(publisher.publish().unwrap(), u64::from(generation));
        assert!(publisher.fill(99).is_err());
        assert_eq!(monitor.snapshot().active, 1);
        process.ready(u64::from(generation));
        process.command(if generation == 3 { "finish" } else { "return" });
        process.publisher.as_mut().unwrap().reclaim().unwrap();
    }
    assert_eq!(monitor.snapshot().completed, 3);
    assert_eq!(monitor.snapshot().active, 0);
    assert!(process.child.wait().unwrap().success());
    assert_eq!(std::fs::read_dir(&process.root).unwrap().count(), 2);
}
#[test]
fn death() {
    let mut process = Process::start();
    process.publisher.as_mut().unwrap().fill(1).unwrap();
    process.publisher.as_mut().unwrap().publish().unwrap();
    process.ready(1);
    process.command("exit");
    assert!(process.child.wait().unwrap().success());
    let publisher = process.publisher.as_mut().unwrap();
    assert!(publisher.reclaim().is_err());
    assert_eq!(publisher.phase(), Phase::Retired);
    assert!(publisher.fill(2).is_err());
    assert_eq!(publisher.monitor().snapshot().active, 0);
    assert_eq!(publisher.monitor().snapshot().failed, 1);
}
#[test]
fn retention() {
    let mut process = Process::start();
    process.publisher.as_mut().unwrap().fill(1).unwrap();
    process.publisher.as_mut().unwrap().publish().unwrap();
    process.ready(1);
    drop(process.publisher.take());
    process.command("survive");
    process.ready(1);
    assert!(process.child.wait().unwrap().success());
}
#[test]
fn stale() {
    let root = std::env::temp_dir().join(format!("ipc-stale-{}", std::process::id()));
    let (left, mut right) = UnixStream::pair().unwrap();
    let mut publisher = Publisher::new(left, &root, 16).unwrap();
    publisher.publish().unwrap();
    right.write_all(b"PLRET001\0\0\0\0\0\0\0\0").unwrap();
    assert!(publisher.reclaim().is_err());
    assert_eq!(publisher.phase(), Phase::Retired);
    std::fs::remove_dir(&root).unwrap();
}

#[test]
fn disconnected() {
    let mut process = Process::start();
    process.publisher.as_mut().unwrap().fill(1).unwrap();
    process.publisher.as_mut().unwrap().publish().unwrap();
    process.ready(1);
    process.command("disconnect");
    let publisher = process.publisher.as_mut().unwrap();
    assert!(publisher.reclaim().is_err());
    assert!(publisher.fill(2).is_err());
    process.command("check");
    process.ready(1);
    assert!(process.child.wait().unwrap().success());
}
