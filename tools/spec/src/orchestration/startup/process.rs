use super::{MILESTONES, Study, TIMEOUT, absolute, gather, nanos, summarize, terminate};
use crate::Result;
use crate::orchestration::{digest, platform, write};
use serde_json::{Map, Value, json};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Instant;
use wait_timeout::ChildExt;

impl Study<'_> {
    pub(super) fn scenarios(&self) -> Result<Value> {
        let binary = absolute(self.args.one("--binary").unwrap_or("target/release/spec"))?;
        let name = self.args.one("--scenario").unwrap_or("cards");
        let destination = self.destination;
        write(
            &destination.join("manifest.json"),
            &json!({
                "mode": "scenario", "binary": binary, "sha256": digest(&binary)?,
                "scenario": name, "runs": self.runs, "platform": platform(self.host),
                "native_window": false, "focus": "absent",
            }),
        )?;
        let mut samples = Vec::new();
        for index in 0..self.runs {
            eprintln!("Startup {}/{}", index + 1, self.runs);
            samples.push(scenario(&binary, name, destination, index)?);
            write(&destination.join("samples.json"), &json!(samples))?;
        }
        let summary = consistency(&samples, self.runs)?;
        write(&destination.join("summary.json"), &summary)?;
        Ok(summary)
    }
}

fn scenario(binary: &Path, name: &str, destination: &Path, index: usize) -> Result<Value> {
    let stderr = File::create(destination.join(format!("{index}.stderr")))
        .map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut child = Command::new(binary)
        .args(["startup", name])
        .stdout(Stdio::piped())
        .stderr(stderr)
        .spawn()
        .map_err(|error| error.to_string())?;
    let pid = child.id();
    let stdout = child
        .stdout
        .take()
        .ok_or("startup stdout was not captured")?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let reader = std::thread::spawn(move || -> Result<(String, String)> {
        let mut reader = BufReader::new(stdout);
        let mut first = String::new();
        reader
            .read_line(&mut first)
            .map_err(|error| error.to_string())?;
        sender
            .send(first.clone())
            .map_err(|error| error.to_string())?;
        let mut remainder = String::new();
        reader
            .read_to_string(&mut remainder)
            .map_err(|error| error.to_string())?;
        Ok((first, remainder))
    });
    let first = match receiver.recv_timeout(TIMEOUT) {
        Ok(first) if !first.is_empty() => first,
        Ok(_) => {
            return terminate(
                &mut child,
                "startup closed stdout before its first-frame signal",
            );
        }
        Err(_) => return terminate(&mut child, "startup did not produce a first-frame signal"),
    };
    let ready = nanos(started.elapsed());
    let status = child
        .wait_timeout(TIMEOUT)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "startup did not exit after its first-frame signal".to_owned());
    let status = match status {
        Ok(status) => status,
        Err(error) => return terminate(&mut child, &error),
    };
    let (captured, remainder) = reader
        .join()
        .map_err(|_| "startup output reader panicked".to_owned())??;
    std::fs::write(
        destination.join(format!("{index}.stdout")),
        captured + &remainder,
    )
    .map_err(|error| error.to_string())?;
    if !status.success() {
        return Err(format!("startup exited {status}; see {index}.stderr"));
    }
    Ok(json!({
        "pid": pid, "ready": ready,
        "first": serde_json::from_str::<Value>(&first).map_err(|error| error.to_string())?,
        "checked": serde_json::from_str::<Value>(&remainder).map_err(|error| error.to_string())?,
    }))
}

fn consistency(samples: &[Value], runs: usize) -> Result<Value> {
    let checked = samples
        .first()
        .and_then(|sample| sample.get("checked"))
        .ok_or("startup samples are empty")?;
    for sample in samples {
        let next = &sample["checked"];
        if next["checksum"] != checked["checksum"] {
            return Err("startup output changed across fresh processes".into());
        }
        if next["assets"] != checked["assets"] {
            return Err("startup assets changed across fresh processes".into());
        }
    }
    let samples: Vec<_> = samples.iter().collect();
    let mut columns = Map::new();
    for key in ["device", "content", "preparation", "completion", "first"] {
        columns.insert(key.into(), gather(&samples, &["first", key])?);
    }
    for key in MILESTONES {
        columns.insert(key.into(), gather(&samples, &["first", "milestones", key])?);
    }
    columns.insert("ready".into(), gather(&samples, &["ready"])?);
    for key in ["driver", "library", "states"] {
        columns.insert(key.into(), gather(&samples, &["first", "setup", key])?);
    }
    Ok(json!({
        "unit": "nanoseconds", "samples": runs, "summary": summarize(columns)?,
        "cache": "fresh application process; OS file and driver caches uncontrolled",
        "endpoint": "ready includes spawn/loader and delivery of first GPU-completed signal; CPU verification/readback follows signal",
        "milestones": "page-relative; presentation is the first successful frame acknowledgement after GPU completion",
        "provider": "app://scenarios mapped to local files; no HTTP network roundtrip",
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifacts() {
        let milestones = json!({"loaded": 1, "runtime": 2, "activation": 3, "scene": 4,
            "publication": 5, "frame": 6, "presentation": 7});
        let setup = json!({"driver": 8, "library": 9, "states": 10});
        let sample = |ready, checksum| {
            json!({
                "ready": ready,
                "first": {
                    "device": 1, "content": 2, "preparation": 3, "completion": 4, "first": 5,
                    "milestones": milestones, "setup": setup,
                },
                "checked": {"checksum": checksum, "assets": ["one"]}
            })
        };
        let summary = consistency(&[sample(11, "same"), sample(13, "same")], 2).unwrap();
        assert_eq!(summary["summary"]["ready"]["median"], 12.0);
        assert!(consistency(&[sample(11, "one"), sample(12, "two")], 2).is_err());
    }
}
