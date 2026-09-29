use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;

static SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct Process {
    pub child: Child,
    pub root: PathBuf,
    pub directory: PathBuf,
    pub socket: PathBuf,
}

impl Process {
    pub fn start() -> Self {
        let root = PathBuf::from(format!(
            "/tmp/plectron-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let script = root.join("main.js");
        std::fs::write(
            &script,
            fixtures::script("platform, console", "let count = 10;"),
        )
        .unwrap();
        let directory = root.join("control");
        let child = Command::new(env!("CARGO_BIN_EXE_plectron"))
            .args(["run", "--wait"])
            .arg(script)
            .env("PLECTRON_CONTROL_DIR", &directory)
            .env("PLECTRON_RENDERER", "cpu")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let socket = directory.join(format!("{}.sock", child.id()));
        let mut process = Self {
            child,
            root,
            directory,
            socket,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while UnixStream::connect(&process.socket).is_err() {
            assert!(process.child.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline, "IPC startup timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(process.control(&["status"])["pid"], process.child.id());
        process
    }

    pub fn command(&self, arguments: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_plectron"))
            .env("PLECTRON_CONTROL_DIR", &self.directory)
            .args(arguments)
            .output()
            .unwrap()
    }

    pub fn control(&self, arguments: &[&str]) -> Value {
        let pid = self.child.id().to_string();
        let mut command = vec!["ctl", &pid];
        command.extend_from_slice(arguments);
        let output = self.command(&command);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["result"]["Ok"].clone()
    }

    pub fn evaluate(&self, source: &str) -> Value {
        let accepted = self.control(&["eval", source]);
        assert_eq!(accepted["type"], "accepted");
        self.completed(&accepted["job"])
    }

    pub fn completed(&self, id: &Value) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let snapshot = self.control(&["control"]);
            let job = snapshot["snapshot"]["jobs"]
                .as_array()
                .unwrap()
                .iter()
                .find(|job| &job["id"] == id)
                .unwrap();
            if job["phase"] == "complete" {
                return job["result"].clone();
            }
            assert!(Instant::now() < deadline, "control completion timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn raw(&self, bytes: &[u8]) -> Value {
        let mut stream = UnixStream::connect(&self.socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream.write_all(bytes).unwrap();
        let mut response = String::new();
        stream.read_to_string(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }

    pub fn finish(&mut self) {
        assert_eq!(self.control(&["shutdown"])["type"], "shutdown");
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < deadline, "IPC shutdown timed out");
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(!self.socket.exists());
        let output = self.command(&["ps"]);
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
