use super::summary::{coverage, lifecycle};
use super::{Study, TIMEOUT, absolute, nanos};
use crate::Result;
use crate::orchestration::{digest, platform, write};
use serde_json::{Value, json};
use std::fs::File;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

struct Launch<'a> {
    binary: &'a Path,
    root: &'a Path,
    destination: &'a Path,
    visibility: f64,
}

struct Control<'a> {
    binary: &'a Path,
    directory: &'a Path,
    pid: u32,
}

impl Study<'_> {
    pub(super) fn applications(&self, root: &Path) -> Result<Value> {
        let binary = absolute(
            self.args
                .one("--plectron")
                .unwrap_or("target/release/plectron"),
        )?;
        let root = absolute(root.to_str().ok_or("application path is not UTF-8")?)?;
        let backends = match self.args.many("--backends") {
            values if values.is_empty() => vec!["cpu", "metal"],
            values => values,
        };
        if backends
            .iter()
            .any(|backend| !matches!(*backend, "cpu" | "metal"))
        {
            return Err("backends must be cpu or metal".into());
        }
        let visibility = self
            .args
            .one("--visibility-wait")
            .unwrap_or("2.0")
            .parse::<f64>()
            .map_err(|_| "visibility wait must be a number".to_owned())?;
        if !visibility.is_finite() || visibility < 0.0 {
            return Err("visibility wait cannot be negative".into());
        }
        let destination = self.destination;
        write(
            &destination.join("manifest.json"),
            &json!({
                "mode": "application", "binary": binary, "sha256": digest(&binary)?,
                "application": root, "backends": backends, "runs": self.runs,
                "platform": platform(self.host), "native_window": true,
                "focus": "incidental and unobserved", "visibility_wait_seconds": visibility,
            }),
        )?;
        let launch = Launch {
            binary: &binary,
            root: &root,
            destination,
            visibility,
        };
        let mut samples = Vec::new();
        for backend in &backends {
            for index in 0..self.runs {
                eprintln!("{backend} startup {}/{}", index + 1, self.runs);
                samples.push(launch.sample(backend, index)?);
                write(&destination.join("samples.json"), &json!(samples))?;
            }
        }
        let summary = coverage(&samples, &backends, self.runs)?;
        write(&destination.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

impl Launch<'_> {
    fn sample(&self, backend: &str, index: usize) -> Result<Value> {
        let control = tempfile::Builder::new()
            .prefix("plectron-startup-")
            .tempdir()
            .map_err(|error| error.to_string())?;
        let stdout = File::create(self.destination.join(format!("{backend}-{index}.stdout")))
            .map_err(|error| error.to_string())?;
        let log = self.destination.join(format!("{backend}-{index}.stderr"));
        let stderr = File::create(&log).map_err(|error| error.to_string())?;
        let started = Instant::now();
        let mut child = Command::new(self.binary)
            .args(["dev", "start"])
            .arg(self.root)
            .env("PLECTRON_CONTROL_DIR", control.path())
            .env("PLECTRON_RENDERER", backend)
            .stdout(stdout)
            .stderr(stderr)
            .spawn()
            .map_err(|error| error.to_string())?;
        let channel = Control {
            binary: self.binary,
            directory: control.path(),
            pid: child.id(),
        };
        let wait = if backend == "metal" {
            self.visibility
        } else {
            0.0
        };
        let result = channel
            .observe(&mut child, started, wait)
            .and_then(|mut sample| {
                sample["backend"] = json!(backend);
                channel.request(&["shutdown"], true)?;
                let status = child
                    .wait_timeout(TIMEOUT)
                    .map_err(|error| error.to_string())?
                    .ok_or("application did not exit after shutdown")?;
                if !status.success() {
                    return Err(format!(
                        "application exited {status}; see {}",
                        log.display()
                    ));
                }
                Ok(sample)
            });
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            let _ = child.kill();
            let _ = child.wait();
        }
        result
    }
}

impl Control<'_> {
    fn observe(&self, child: &mut Child, started: Instant, wait: f64) -> Result<Value> {
        let (frame, observed) = self.first(child, started, wait)?;
        let page = self
            .request(&["page", "1"], true)?
            .ok_or("page request returned no result")?;
        Ok(json!({
            "pid": self.pid, "backend": Value::Null, "observed": observed,
            "lifecycle": lifecycle(&frame)?, "frame": frame,
            "milestones": page["snapshot"]["milestones"],
        }))
    }

    fn first(&self, child: &mut Child, started: Instant, wait: f64) -> Result<(Value, u64)> {
        let query = r#"{"type":"read","record":1}"#;
        let deadline = Instant::now() + TIMEOUT;
        let mut cutoff = None;
        while Instant::now() < deadline {
            if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
                return Err(format!(
                    "application exited {status} before its first frame"
                ));
            }
            let reply = self.request(&["query", query], false)?;
            if let Some(record) = examine(reply.as_ref(), wait, &mut cutoff)? {
                return Ok((record, nanos(started.elapsed())));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        Err("application did not complete its first frame".into())
    }

    fn request(&self, arguments: &[&str], check: bool) -> Result<Option<Value>> {
        let output = Command::new(self.binary)
            .arg("ctl")
            .arg(self.pid.to_string())
            .args(arguments)
            .env("PLECTRON_CONTROL_DIR", self.directory)
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            if !check {
                return Ok(None);
            }
            let message = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            return Err(if message.is_empty() {
                "control request failed".into()
            } else {
                message
            });
        }
        let response: Value =
            serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
        let result = &response["result"];
        if let Some(error) = result.get("Err") {
            return Err(error.to_string());
        }
        Ok(result.get("Ok").cloned())
    }
}

fn examine(
    reply: Option<&Value>,
    wait: f64,
    cutoff: &mut Option<Instant>,
) -> Result<Option<Value>> {
    let Some(reply) = reply else {
        return Ok(None);
    };
    let status = &reply["reply"]["status"];
    let state = &status["record"]["state"]["type"];
    match status["type"].as_str() {
        Some("record") if state == "complete" => complete(&status["record"], wait, cutoff),
        Some("record") if state != "pending" => Err(format!("first frame ended as {state}")),
        Some("evicted") => Err("first-frame record was evicted before observation".into()),
        _ => Ok(None),
    }
}

fn complete(record: &Value, wait: f64, cutoff: &mut Option<Instant>) -> Result<Option<Value>> {
    if let Some(error) = record["state"]["result"].get("Err") {
        return Err(error.to_string());
    }
    if record["target"]["type"] != "frame" || record["target"]["unit"] != 1 {
        return Err("record 1 is not the first content frame".into());
    }
    if wait > 0.0 && record["visible"].is_null() {
        let end = *cutoff.get_or_insert_with(|| Instant::now() + Duration::from_secs_f64(wait));
        if Instant::now() < end {
            return Ok(None);
        }
    }
    Ok(Some(record.clone()))
}
