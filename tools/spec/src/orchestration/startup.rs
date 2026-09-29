use super::{Arguments, digest, output, platform, stats, write};
use crate::Result;
use serde_json::{Map, Value, json};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use wait_timeout::ChildExt;

const TIMEOUT: Duration = Duration::from_secs(60);
const MILESTONES: [&str; 7] = [
    "loaded",
    "runtime",
    "activation",
    "scene",
    "publication",
    "frame",
    "presentation",
];

pub fn run(values: &[String]) -> Result<Value> {
    let args = Arguments::new(values);
    args.reject(&[
        "--binary",
        "--scenario",
        "--application",
        "--native-window",
        "--plectron",
        "--backends",
        "--runs",
        "--visibility-wait",
        "--output",
    ])?;
    let runs = number(args.one("--runs").unwrap_or("5"), "runs")?;
    if runs == 0 {
        return Err("runs must be positive".into());
    }
    let destination = args.one("--output").ok_or("--output is required")?;
    let application = args.one("--application");
    let native = values.iter().any(|value| value == "--native-window");
    if application.is_some() != native {
        return Err(if application.is_some() {
            "--application opens native windows and requires --native-window".into()
        } else {
            "--native-window requires --application".into()
        });
    }
    let destination = output(Path::new(destination))?;
    match (application, native) {
        (Some(root), true) => applications(&args, Path::new(root), &destination, runs),
        (None, false) => scenarios(&args, &destination, runs),
        _ => unreachable!(),
    }
}

fn scenarios(args: &Arguments<'_>, destination: &Path, runs: usize) -> Result<Value> {
    let binary = absolute(args.one("--binary").unwrap_or("target/release/spec"))?;
    let name = args.one("--scenario").unwrap_or("cards");
    write(
        &destination.join("manifest.json"),
        &json!({
            "mode": "scenario", "binary": binary, "sha256": digest(&binary)?,
            "scenario": name, "runs": runs, "platform": platform(),
            "native_window": false, "focus": "absent",
        }),
    )?;
    let mut samples = Vec::new();
    for index in 0..runs {
        eprintln!("Startup {}/{}", index + 1, runs);
        samples.push(scenario(&binary, name, destination, index)?);
        write(&destination.join("samples.json"), &json!(samples))?;
    }
    scenario_summary(&samples, runs).and_then(|summary| {
        write(&destination.join("summary.json"), &summary)?;
        Ok(summary)
    })
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

fn scenario_summary(samples: &[Value], runs: usize) -> Result<Value> {
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
    let mut columns = Map::new();
    for key in ["device", "content", "preparation", "completion", "first"] {
        columns.insert(key.into(), collect(samples, &["first", key])?);
    }
    for key in MILESTONES {
        columns.insert(key.into(), collect(samples, &["first", "milestones", key])?);
    }
    columns.insert("ready".into(), collect(samples, &["ready"])?);
    for key in ["driver", "library", "states"] {
        columns.insert(key.into(), collect(samples, &["first", "setup", key])?);
    }
    Ok(json!({
        "unit": "nanoseconds", "samples": runs, "summary": summarize(columns)?,
        "cache": "fresh application process; OS file and driver caches uncontrolled",
        "endpoint": "ready includes spawn/loader and delivery of first GPU-completed signal; CPU verification/readback follows signal",
        "milestones": "page-relative; presentation is the first successful frame acknowledgement after GPU completion",
        "provider": "app://scenarios mapped to local files; no HTTP network roundtrip",
    }))
}

fn applications(
    args: &Arguments<'_>,
    root: &Path,
    destination: &Path,
    runs: usize,
) -> Result<Value> {
    let binary = absolute(args.one("--plectron").unwrap_or("target/release/plectron"))?;
    let root = absolute(root.to_str().ok_or("application path is not UTF-8")?)?;
    let backends = match args.many("--backends") {
        values if values.is_empty() => vec!["cpu", "metal"],
        values => values,
    };
    if backends
        .iter()
        .any(|backend| !matches!(*backend, "cpu" | "metal"))
    {
        return Err("backends must be cpu or metal".into());
    }
    let visibility = args
        .one("--visibility-wait")
        .unwrap_or("2.0")
        .parse::<f64>()
        .map_err(|_| "visibility wait must be a number".to_owned())?;
    if !visibility.is_finite() || visibility < 0.0 {
        return Err("visibility wait cannot be negative".into());
    }
    write(
        &destination.join("manifest.json"),
        &json!({
            "mode": "application", "binary": binary, "sha256": digest(&binary)?,
            "application": root, "backends": backends, "runs": runs,
            "platform": platform(), "native_window": true,
            "focus": "incidental and unobserved", "visibility_wait_seconds": visibility,
        }),
    )?;
    let mut samples = Vec::new();
    for backend in &backends {
        for index in 0..runs {
            eprintln!("{backend} startup {}/{}", index + 1, runs);
            samples.push(application(
                &binary,
                &root,
                backend,
                destination,
                index,
                visibility,
            )?);
            write(&destination.join("samples.json"), &json!(samples))?;
        }
    }
    let summary = application_summary(&samples, &backends, runs)?;
    write(&destination.join("summary.json"), &summary)?;
    Ok(summary)
}

fn application(
    binary: &Path,
    root: &Path,
    backend: &str,
    destination: &Path,
    index: usize,
    visibility: f64,
) -> Result<Value> {
    let control = tempfile::Builder::new()
        .prefix("plectron-startup-")
        .tempdir()
        .map_err(|error| error.to_string())?;
    let stdout = File::create(destination.join(format!("{backend}-{index}.stdout")))
        .map_err(|error| error.to_string())?;
    let stderr_path = destination.join(format!("{backend}-{index}.stderr"));
    let stderr = File::create(&stderr_path).map_err(|error| error.to_string())?;
    let started = Instant::now();
    let mut child = Command::new(binary)
        .args(["dev", "start"])
        .arg(root)
        .env("PLECTRON_CONTROL_DIR", control.path())
        .env("PLECTRON_RENDERER", backend)
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|error| error.to_string())?;
    let pid = child.id();
    let result = (|| {
        let wait = if backend == "metal" { visibility } else { 0.0 };
        let (frame, observed) = first(binary, control.path(), &mut child, started, wait)?;
        let page = control_request(binary, control.path(), pid, &["page", "1"], true)?
            .ok_or("page request returned no result")?;
        let sample = json!({
            "pid": pid, "backend": backend, "observed": observed,
            "lifecycle": lifecycle(&frame)?, "frame": frame,
            "milestones": page["snapshot"]["milestones"],
        });
        control_request(binary, control.path(), pid, &["shutdown"], true)?;
        let status = child
            .wait_timeout(TIMEOUT)
            .map_err(|error| error.to_string())?
            .ok_or("application did not exit after shutdown")?;
        if !status.success() {
            return Err(format!(
                "application exited {status}; see {}",
                stderr_path.display()
            ));
        }
        Ok(sample)
    })();
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

fn first(
    binary: &Path,
    directory: &Path,
    child: &mut Child,
    started: Instant,
    visibility: f64,
) -> Result<(Value, u64)> {
    let pid = child.id();
    let query = r#"{"type":"read","record":1}"#;
    let deadline = Instant::now() + TIMEOUT;
    let mut visibility_deadline = None;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            return Err(format!(
                "application exited {status} before its first frame"
            ));
        }
        if let Some(reply) = control_request(binary, directory, pid, &["query", query], false)? {
            let status = &reply["reply"]["status"];
            match status["type"].as_str() {
                Some("record") if status["record"]["state"]["type"] == "complete" => {
                    let record = &status["record"];
                    if let Some(error) = record["state"]["result"].get("Err") {
                        return Err(error.to_string());
                    }
                    if record["target"]["type"] != "frame" || record["target"]["unit"] != 1 {
                        return Err("record 1 is not the first content frame".into());
                    }
                    if visibility > 0.0 && record["visible"].is_null() {
                        let end = *visibility_deadline.get_or_insert_with(|| {
                            Instant::now() + Duration::from_secs_f64(visibility)
                        });
                        if Instant::now() < end {
                            std::thread::sleep(Duration::from_millis(5));
                            continue;
                        }
                    }
                    return Ok((record.clone(), nanos(started.elapsed())));
                }
                Some("record") if status["record"]["state"]["type"] != "pending" => {
                    return Err(format!(
                        "first frame ended as {}",
                        status["record"]["state"]["type"]
                    ));
                }
                Some("evicted") => {
                    return Err("first-frame record was evicted before observation".into());
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    Err("application did not complete its first frame".into())
}

fn control_request(
    binary: &Path,
    directory: &Path,
    pid: u32,
    arguments: &[&str],
    check: bool,
) -> Result<Option<Value>> {
    let output = Command::new(binary)
        .arg("ctl")
        .arg(pid.to_string())
        .args(arguments)
        .env("PLECTRON_CONTROL_DIR", directory)
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

fn lifecycle(record: &Value) -> Result<Value> {
    let admitted = integer(record, "admitted")?;
    let produced = integer(record, "produced")?;
    let detected = integer(record, "detected")?;
    if admitted > produced || produced > detected {
        return Err("first-frame timestamps are not monotonic".into());
    }
    let visible = record.get("visible").and_then(Value::as_u64);
    let seen = record.get("seen").and_then(Value::as_u64);
    if let Some(visible) = visible {
        let seen = seen.ok_or("visible first frame has no seen timestamp")?;
        if produced > visible || visible > seen {
            return Err("first-frame visibility timestamps are not monotonic".into());
        }
    }
    Ok(json!({
        "submitted": admitted, "backend_completed": produced,
        "visible": visible, "seen": seen, "drops": record["drops"],
        "interactive": detected, "submit_to_backend": produced - admitted,
        "backend_to_interactive": detected - produced,
        "backend_to_visible": visible.map(|value| value - produced),
        "visible_to_detection": visible.zip(seen).map(|(visible, seen)| seen - visible),
    }))
}

fn application_summary(samples: &[Value], backends: &[&str], runs: usize) -> Result<Value> {
    let mut summaries = Map::new();
    for backend in backends {
        let selected = samples
            .iter()
            .filter(|sample| sample["backend"] == *backend)
            .collect::<Vec<_>>();
        let mut columns = Map::new();
        for key in [
            "submitted",
            "backend_completed",
            "interactive",
            "submit_to_backend",
            "backend_to_interactive",
            "drops",
        ] {
            columns.insert(key.into(), collect_refs(&selected, &["lifecycle", key])?);
        }
        let visible = selected
            .iter()
            .copied()
            .filter(|sample| !sample["lifecycle"]["visible"].is_null())
            .collect::<Vec<_>>();
        if !visible.is_empty() {
            for key in [
                "visible",
                "seen",
                "backend_to_visible",
                "visible_to_detection",
            ] {
                columns.insert(key.into(), collect_refs(&visible, &["lifecycle", key])?);
            }
        }
        columns.insert("observed".into(), collect_refs(&selected, &["observed"])?);
        for name in MILESTONES {
            columns.insert(name.into(), collect_refs(&selected, &["milestones", name])?);
        }
        for pair in MILESTONES.windows(2) {
            let values = selected
                .iter()
                .map(|sample| {
                    let start = sample["milestones"][pair[0]].as_u64().ok_or_else(|| {
                        format!("first frame is missing the {} milestone", pair[0])
                    })?;
                    let end = sample["milestones"][pair[1]].as_u64().ok_or_else(|| {
                        format!("first frame is missing the {} milestone", pair[1])
                    })?;
                    Ok(json!(end - start))
                })
                .collect::<Result<Vec<_>>>()?;
            columns.insert(format!("{}_to_{}", pair[0], pair[1]), Value::Array(values));
        }
        let mut summary = summarize(columns)?;
        summary.as_object_mut().unwrap().insert(
            "visibility_coverage".into(),
            json!({"observed": visible.len(), "total": selected.len()}),
        );
        summaries.insert((*backend).into(), summary);
    }
    Ok(json!({
        "unit": "nanoseconds", "samples_per_backend": runs, "summary": summaries,
        "cache": "fresh application process; OS file and driver caches uncontrolled",
        "clock": "submitted, backend_completed, visible and interactive share the platform lifetime monotonic clock",
        "milestones": "loaded through presentation share the page-relative monotonic clock",
        "endpoint": {
            "submitted": "first frame admitted immediately before backend rendering",
            "backend_completed": "backend completion timestamp; Metal includes its presentation copy",
            "visible": "physical drawable presentation when the platform supplies evidence; absent otherwise",
            "interactive": "backend completion consumed and published to the page; logical input can be accepted independently of visibility",
            "observed": "parent-process detection through passive IPC; polling makes this an upper bound",
        },
    }))
}

fn summarize(columns: Map<String, Value>) -> Result<Value> {
    columns
        .into_iter()
        .map(|(key, value)| {
            let values = value.as_array().ok_or("summary column is not an array")?;
            Ok((key, stats(values)?))
        })
        .collect::<Result<Map<_, _>>>()
        .map(Value::Object)
}

fn collect(samples: &[Value], path: &[&str]) -> Result<Value> {
    collect_refs(&samples.iter().collect::<Vec<_>>(), path)
}

fn collect_refs(samples: &[&Value], path: &[&str]) -> Result<Value> {
    samples
        .iter()
        .map(|sample| {
            path.iter()
                .try_fold(*sample, |value, key| {
                    value.get(key).ok_or_else(|| format!("missing {key}"))
                })
                .cloned()
        })
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

fn absolute(path: &str) -> Result<PathBuf> {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        Ok(path)
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())
            .map(|directory| directory.join(path))
    }
}

fn integer(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("first frame has no {key} timestamp"))
}

fn nanos(duration: Duration) -> u64 {
    duration.as_nanos().try_into().unwrap_or(u64::MAX)
}

fn number(value: &str, name: &str) -> Result<usize> {
    value
        .parse()
        .map_err(|_| format!("{name} must be an integer"))
}

fn terminate<T>(child: &mut Child, message: &str) -> Result<T> {
    let _ = child.kill();
    let _ = child.wait();
    Err(message.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_summary_checks_artifacts_and_distributions() {
        let sample = |ready, checksum| {
            json!({
                "ready": ready,
                "first": {
                    "device": 1, "content": 2, "preparation": 3, "completion": 4, "first": 5,
                    "milestones": {"loaded": 1, "runtime": 2, "activation": 3, "scene": 4,
                        "publication": 5, "frame": 6, "presentation": 7},
                    "setup": {"driver": 8, "library": 9, "states": 10}
                },
                "checked": {"checksum": checksum, "assets": ["one"]}
            })
        };
        let summary = scenario_summary(&[sample(11, "same"), sample(13, "same")], 2).unwrap();
        assert_eq!(summary["summary"]["ready"]["median"], 12.0);
        assert!(scenario_summary(&[sample(11, "one"), sample(12, "two")], 2).is_err());
    }

    #[test]
    fn lifecycle_rejects_nonmonotonic_records() {
        let record = json!({"admitted": 3, "produced": 2, "detected": 4, "drops": 0});
        assert!(lifecycle(&record).is_err());
    }

    #[test]
    fn application_summary_keeps_visibility_coverage_and_stage_deltas() {
        let sample = json!({
            "backend": "metal", "observed": 20,
            "lifecycle": {"submitted": 1, "backend_completed": 3, "interactive": 5,
                "submit_to_backend": 2, "backend_to_interactive": 2, "drops": 0,
                "visible": 4, "seen": 5, "backend_to_visible": 1,
                "visible_to_detection": 1},
            "milestones": {"loaded": 1, "runtime": 2, "activation": 4, "scene": 7,
                "publication": 11, "frame": 16, "presentation": 22}
        });
        let summary = application_summary(&[sample], &["metal"], 1).unwrap();
        assert_eq!(
            summary["summary"]["metal"]["loaded_to_runtime"]["median"],
            1
        );
        assert_eq!(
            summary["summary"]["metal"]["visibility_coverage"]["observed"],
            1
        );
    }
}
