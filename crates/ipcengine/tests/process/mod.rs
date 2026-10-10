use ipcengine::{
    endpoint,
    file::Draft,
    message::{self, Connection, Server},
    stream::Transfer,
};
use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

pub struct Child {
    process: std::process::Child,
    _output: BufReader<std::process::ChildStdout>,
    pub root: PathBuf,
    pub pid: u32,
}
impl Child {
    pub fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "plectron-process-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut process = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "service", "--ignored", "--nocapture"])
            .env("PLECTRON_IPC_TEST_ROOT", &root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let pid = process.id();
        let mut output = BufReader::new(process.stdout.take().unwrap());
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = std::thread::spawn(move || {
            let mut line = String::new();
            while output.read_line(&mut line).unwrap() != 0 {
                if line.trim() == format!("PLECTRON_READY {pid}") {
                    let _ = sender.send(output);
                    return;
                }
                line.clear();
            }
        });
        let output = match receiver.recv_timeout(Duration::from_secs(5)) {
            Ok(output) => output,
            Err(error) => {
                let _ = process.kill();
                let _ = process.wait();
                let _ = worker.join();
                let _ = std::fs::remove_dir_all(&root);
                panic!("IPC child never became ready: {error}");
            }
        };
        worker.join().unwrap();
        Self {
            process,
            _output: output,
            root,
            pid,
        }
    }
    pub fn stop(mut self) {
        let mut connection = Connection::connect(&endpoint::path(&self.root, self.pid)).unwrap();
        connection.write(&"stop", 64).unwrap();
        assert_eq!(
            connection.read::<u32>(64, Duration::from_secs(2)).unwrap(),
            42
        );
        drop(connection);
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            if let Some(status) = self.process.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "IPC child did not shut down");
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(!endpoint::path(&self.root, self.pid).exists());
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
pub fn serve() {
    let root = PathBuf::from(std::env::var_os("PLECTRON_IPC_TEST_ROOT").unwrap());
    let storage = root.join("storage");
    let server = Server::bind(&root, std::process::id(), move |mut connection, sender| {
        let Ok(command) = connection.read::<String>(64, Duration::from_secs(2)) else {
            return;
        };
        if command == "file" {
            let mut draft = Draft::create(&storage, 3).unwrap();
            draft.write_all(b"abc").unwrap();
            let artifact = draft.seal().unwrap();
            connection.write(&artifact.length(), 64).unwrap();
            Transfer::new(artifact.reader(), artifact.length()).serve(&mut connection);
        } else {
            let reply = message::ask(&sender, command, None).unwrap();
            connection.write(&reply, 64).unwrap();
        }
    })
    .unwrap();
    println!("PLECTRON_READY {}", std::process::id());
    std::io::stdout().flush().unwrap();
    let request = server
        .requests
        .recv_timeout(Duration::from_secs(10))
        .unwrap();
    assert_eq!(request.request, "stop");
    request.reply.send(42u32).unwrap();
    drop(server);
}
