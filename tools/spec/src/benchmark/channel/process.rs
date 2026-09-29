use super::{Context, Report, error};
use crate::Result;
use ipcengine::message::Connection;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
pub struct Process {
    pub root: PathBuf,
    pub data: Option<UnixStream>,
    child: Option<Child>,
    echo: Option<JoinHandle<()>>,
}
impl Process {
    pub fn new(context: &Context, mode: &str, length: usize, iterations: usize) -> Result<Self> {
        let root = context.temporary.join(format!(
            "plectron-channel-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        ipcengine::endpoint::prepare(&root).map_err(error)?;
        let mut process = Self {
            root,
            data: None,
            child: None,
            echo: None,
        };
        let data = UnixListener::bind(process.root.join("data")).map_err(error)?;
        let control = UnixListener::bind(process.root.join("control")).map_err(error)?;
        process.child = Some(
            Command::new(&context.executable)
                .arg("channel-child")
                .arg(&process.root)
                .arg(mode)
                .arg(length.to_string())
                .arg(iterations.to_string())
                .stdout(Stdio::piped())
                .spawn()
                .map_err(error)?,
        );
        process.data = Some(accept(&data)?);
        let socket = accept(&control)?;
        process.echo = Some(std::thread::spawn(move || echo(socket)));
        Ok(process)
    }
    pub fn finish(&mut self) -> Result<Report> {
        let output = self
            .child
            .take()
            .expect("child process")
            .wait_with_output()
            .map_err(error)?;
        if let Some(echo) = self.echo.take() {
            echo.join().map_err(|_| "echo worker panicked")?;
        }
        if !output.status.success() {
            return Err("channel child failed".into());
        }
        serde_json::from_slice(&output.stdout).map_err(error)
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(echo) = self.echo.take() {
            let _ = echo.join();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn accept(listener: &UnixListener) -> Result<UnixStream> {
    listener.set_nonblocking(true).map_err(error)?;
    let deadline = Instant::now() + Duration::from_secs(6);
    loop {
        match listener.accept() {
            Ok((socket, _)) => return Ok(socket),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(failure) => return Err(error(failure)),
        }
        if Instant::now() >= deadline {
            return Err("channel child startup timed out".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn echo(socket: UnixStream) {
    let Ok(mut connection) = Connection::new(socket) else {
        return;
    };
    while let Ok(nonce) = connection.read::<u64>(64, Duration::from_secs(6)) {
        if connection.write(&nonce, 64).is_err() {
            return;
        }
    }
}
