use crate::Result;
use crate::native;
use serde_json::{Value, json};
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

pub struct Application {
    binary: PathBuf,
    root: PathBuf,
    child: Child,
    socket: PathBuf,
    environment: Vec<(String, String)>,
}

impl Application {
    pub fn start(
        binary: &Path,
        root: &Path,
        renderer: &str,
        project: Option<&Path>,
    ) -> Result<Self> {
        let binary = binary
            .canonicalize()
            .map_err(|error| format!("{}: {error}", binary.display()))?;
        crate::create(root)?;
        let control = root.join("control");
        let environment = vec![
            (
                "PLECTRON_CONTROL_DIR".into(),
                control.to_string_lossy().into(),
            ),
            ("PLECTRON_RENDERER".into(), renderer.into()),
        ];
        let mut command = Command::new(&binary);
        if let Some(project) = project {
            command
                .args(["dev", "start"])
                .arg(project.canonicalize().map_err(|error| error.to_string())?);
        } else {
            let source = root.join("main.js");
            std::fs::write(&source, "let count = 0;").map_err(|error| error.to_string())?;
            command.args(["run", "--wait"]).arg(source);
        }
        let stdout = File::create(root.join("stdout.log")).map_err(|error| error.to_string())?;
        let stderr = File::create(root.join("stderr.log")).map_err(|error| error.to_string())?;
        let child = command
            .envs(environment.iter().cloned())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .spawn()
            .map_err(|error| error.to_string())?;
        let socket = control.join(format!("{}.sock", child.id()));
        Ok(Self {
            binary,
            root: root.into(),
            child,
            socket,
            environment,
        })
    }

    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    pub fn ready(&mut self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.socket.exists() {
            if self
                .child
                .try_wait()
                .map_err(|error| error.to_string())?
                .is_some()
                || Instant::now() >= deadline
            {
                return Err(format!(
                    "application startup failed: {}",
                    self.root.display()
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
        while Instant::now() < deadline {
            let state = self.request(&json!({"type": "control"}))?;
            let jobs = state["snapshot"]["jobs"]
                .as_array()
                .ok_or("control jobs must be an array")?;
            if jobs
                .iter()
                .any(|job| job["kind"] == "startup" && job["phase"] == "complete")
            {
                if jobs.iter().any(|job| !job["result"]["Err"].is_null()) {
                    return Err(format!("startup rejected: {jobs:?}"));
                }
                return Ok(());
            }
        }
        Err("startup completion timed out".into())
    }

    pub fn request(&self, command: &Value) -> Result<Value> {
        let mut connection =
            UnixStream::connect(&self.socket).map_err(|error| error.to_string())?;
        connection
            .set_read_timeout(Some(Duration::from_secs(6)))
            .map_err(|error| error.to_string())?;
        connection
            .set_write_timeout(Some(Duration::from_secs(6)))
            .map_err(|error| error.to_string())?;
        let mut message = serde_json::to_vec(&json!({"version": 1, "command": command}))
            .map_err(|error| error.to_string())?;
        message.push(b'\n');
        connection
            .write_all(&message)
            .map_err(|error| error.to_string())?;
        let mut response = Vec::new();
        BufReader::new(connection)
            .take(8_388_608)
            .read_until(b'\n', &mut response)
            .map_err(|error| error.to_string())?;
        let response: Value =
            serde_json::from_slice(&response).map_err(|error| error.to_string())?;
        if response["version"] != 1 || response["result"].get("Ok").is_none() {
            return Err(format!("IPC rejected {command}: {response}"));
        }
        Ok(response["result"]["Ok"].clone())
    }

    pub fn evaluate(&self, source: &str) -> Result<Value> {
        let start = Instant::now();
        let accepted = self.request(&json!({"type": "eval", "source": source}))?;
        let admission = start.elapsed().as_nanos();
        let mut polls = 0;
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            let state = self.request(&json!({"type": "control"}))?["snapshot"].clone();
            polls += 1;
            let job = state["jobs"]
                .as_array()
                .and_then(|jobs| jobs.iter().find(|job| job["id"] == accepted["job"]))
                .ok_or("accepted job is absent")?;
            if job["phase"] == "complete" {
                if job["result"].get("Ok").is_none() {
                    return Err(format!("job failed: {job}"));
                }
                return Ok(json!({
                    "wall": start.elapsed().as_nanos(),
                    "admission": admission,
                    "polls": polls,
                    "result": job["result"]["Ok"],
                }));
            }
        }
        Err("job completion timed out".into())
    }

    pub fn observation(&self, command: &Value) -> Result<Value> {
        Ok(self.request(&json!({"type": "observe", "command": command}))?["status"].clone())
    }

    pub fn export(&self, identity: i64) -> Result<PathBuf> {
        let target = self.root.join("observation.json");
        self.observation(&json!({"type": "stop", "id": identity}))?;
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.observation(&json!({"type": "status"}))?["open"]
            .as_i64()
            .unwrap_or(0)
            != 0
        {
            if Instant::now() >= deadline {
                return Err("observation did not drain".into());
            }
        }
        let mut command = Command::new(&self.binary);
        command
            .args([
                "observe",
                &self.child.id().to_string(),
                &identity.to_string(),
            ])
            .arg(&target)
            .envs(self.environment.iter().cloned());
        native::invoke(
            &mut command,
            &self.root.join("export.log"),
            Duration::from_secs(15),
        )?;
        self.observation(&json!({"type": "discard", "id": identity}))?;
        Ok(target)
    }

    pub fn close(mut self) -> Result<()> {
        let result = self.close_inner();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        result
    }

    fn close_inner(&mut self) -> Result<()> {
        if self
            .child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            let result = self.request(&json!({"type": "shutdown"}))?;
            if result["type"] != "shutdown"
                || result["errors"]
                    .as_array()
                    .is_none_or(|errors| !errors.is_empty())
            {
                return Err(format!("shutdown failed: {result}"));
            }
            if self
                .child
                .wait_timeout(Duration::from_secs(10))
                .map_err(|error| error.to_string())?
                .is_none()
            {
                return Err("application shutdown timed out".into());
            }
        }
        if !self
            .child
            .wait()
            .map_err(|error| error.to_string())?
            .success()
            || self.socket.exists()
        {
            return Err(format!("unclean application exit: {}", self.root.display()));
        }
        Ok(())
    }
}
