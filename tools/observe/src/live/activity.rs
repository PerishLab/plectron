use crate::application::Application;
use crate::{Output, Result, absolute, digest, frames, integer, ipc, write};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

#[derive(clap::Args)]
pub struct Config {
    #[arg(long)]
    pub binary: PathBuf,
    #[arg(long)]
    pub application: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, num_args = 1.., value_parser = ["cpu", "metal"], default_values_t = ["cpu".to_owned(), "metal".to_owned()])]
    pub backends: Vec<String>,
    #[arg(long, default_value_t = 3)]
    pub rounds: usize,
    #[arg(long, default_value_t = 2.0)]
    pub seconds: f64,
}

pub fn cputime(value: &str) -> Result<i64> {
    let (days, clock) = value.split_once('-').map_or((0, value), |(days, clock)| {
        (days.parse::<i64>().unwrap_or(-1), clock)
    });
    if days < 0 {
        return Err("invalid CPU day count".into());
    }
    let mut parts = clock.split(':').collect::<Vec<_>>();
    let seconds = parts
        .pop()
        .ok_or("CPU time is empty")?
        .parse::<f64>()
        .map_err(|error| error.to_string())?;
    let minutes = parts
        .pop()
        .map(str::parse::<i64>)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(0);
    let hours = parts
        .pop()
        .map(str::parse::<i64>)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or(0);
    if !parts.is_empty() {
        return Err("invalid CPU time".into());
    }
    Ok(
        ((days * 86_400 + hours * 3_600 + minutes * 60) as f64 * 1_000_000_000.0
            + seconds * 1_000_000_000.0)
            .round() as i64,
    )
}

pub fn run(config: &Config) -> Result<Value> {
    crate::application::require()?;
    if config.rounds == 0 || !config.seconds.is_finite() || config.seconds <= 0.0 {
        return Err("rounds and seconds must be positive".into());
    }
    let mut unique = config.backends.clone();
    unique.sort();
    unique.dedup();
    if unique.len() != config.backends.len()
        || config.backends.is_empty()
        || config
            .backends
            .iter()
            .any(|backend| !["cpu", "metal"].contains(&backend.as_str()))
    {
        return Err("backends must be nonempty and unique".into());
    }
    let root = Output::prepare(&config.output)?;
    let host = Host {
        seconds: config.seconds,
        binary: absolute(&config.binary)?,
        project: absolute(&config.application)?,
    };
    let mut rows = Vec::new();
    for index in 0..config.rounds {
        for offset in 0..config.backends.len() {
            let backend = &config.backends[(index + offset) % config.backends.len()];
            eprintln!("[{}/{}] {backend}", index + 1, config.rounds);
            rows.push(host.capture(
                &root.join(format!("{index}-{backend}")),
                backend,
                index % 2 == 1,
            )?);
        }
    }
    let metrics: [(&str, &[&str]); 15] = [
        ("cpu", &["cpu"]),
        ("utilization", &["utilization"]),
        ("rss", &["rss", "end"]),
        ("rss_delta", &["rss", "delta"]),
        ("cache", &["cache", "end"]),
        ("cache_delta", &["cache", "delta"]),
        ("frames", &["frames"]),
        ("animations", &["computation", "animations"]),
        ("effects", &["computation", "effects"]),
        ("coalesced", &["computation", "coalesced"]),
        ("discarded", &["computation", "discarded"]),
        ("submitted", &["computation", "submitted"]),
        ("mailbox_peak", &["computation", "mailbox"]),
        ("sampling", &["computation", "sampling"]),
        ("layouts", &["computation", "layouts"]),
    ];
    let mut summary = Map::new();
    for backend in &config.backends {
        let selected = rows
            .iter()
            .filter(|row| row["backend"] == *backend)
            .collect::<Vec<_>>();
        let mut phases = Map::new();
        for phase in ["animated", "static"] {
            phases.insert(phase.into(), distributions(&selected, phase, &metrics)?);
        }
        summary.insert(backend.clone(), Value::Object(phases));
    }
    let binary = &host.binary;
    let bytes = std::fs::read(binary).map_err(|error| error.to_string())?;
    let result = json!({
        "unit": {"cpu": "nanoseconds", "utilization": "percent", "rss": "bytes", "cache": "bytes", "sampling": "microseconds"},
        "binary": binary, "sha256": digest(&bytes), "application": host.project, "rounds": config.rounds,
        "phase_seconds": config.seconds, "summary": summary, "rows": rows,
        "endpoint": "same-process quiet interval bounded by process CPU/RSS and page snapshots; no IPC during the interval",
        "limits": ["ps reports current resident memory and cumulative process CPU; endpoint samples are not peak RSS.", "Engine cache bytes cover registered acceleration caches and are not process RSS.", "Animated is the focused Todo caret; static moves focus to the Add button.", "Backend completion does not claim hardware scanout."],
    });
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}

fn usage(pid: u32) -> Result<Value> {
    let output = Command::new("ps")
        .args(["-o", "rss=", "-o", "time=", "-p", &pid.to_string()])
        .output()
        .map_err(|error| error.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let values = stdout.split_whitespace().collect::<Vec<_>>();
    if !output.status.success() || values.len() != 2 {
        return Err(format!(
            "process usage unavailable for {pid}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(json!({
        "rss": values[0].parse::<i64>().map_err(|error| error.to_string())? * 1_024,
        "cpu": cputime(values[1])?,
    }))
}

fn counters(snapshot: &Value) -> Result<Value> {
    let cache = snapshot["caches"]
        .as_array()
        .ok_or("caches must be an array")?
        .iter()
        .map(|entry| integer(&entry["stats"]["bytes"], "cache bytes"))
        .sum::<Result<i64>>()?;
    Ok(json!({"frame": snapshot["frame"], "computation": snapshot["computation"], "cache": cache}))
}

fn phase(application: &Application, seconds: f64) -> Result<Value> {
    let before = counters(&frames::page(application)?)?;
    let start = usage(application.pid())?;
    let clock = Instant::now();
    thread::sleep(Duration::from_secs_f64(seconds));
    let end = usage(application.pid())?;
    let wall = clock.elapsed().as_nanos() as i64;
    let after = counters(&frames::page(application)?)?;
    let computation = after["computation"]
        .as_object()
        .ok_or("computation must be an object")?
        .iter()
        .map(|(key, value)| {
            let current = integer(value, key)?;
            let result = if key == "mailbox" {
                current
            } else {
                current - before["computation"][key].as_i64().unwrap_or(0)
            };
            Ok((key.clone(), json!(result)))
        })
        .collect::<Result<Map<_, _>>>()?;
    let cpu = integer(&end["cpu"], "ending CPU")? - integer(&start["cpu"], "starting CPU")?;
    Ok(json!({
        "wall": wall, "cpu": cpu, "utilization": cpu as f64 * 100.0 / wall as f64,
        "rss": {"start": start["rss"], "end": end["rss"], "delta": integer(&end["rss"], "ending RSS")? - integer(&start["rss"], "starting RSS")?},
        "cache": {"start": before["cache"], "end": after["cache"], "delta": integer(&after["cache"], "ending cache")? - integer(&before["cache"], "starting cache")?},
        "frames": integer(&after["frame"], "ending frame")? - integer(&before["frame"], "starting frame")?,
        "computation": computation,
    }))
}

fn focus(application: &Application, previous: &Value, identifier: &str) -> Result<Value> {
    frames::action(
        application,
        previous,
        &format!("document.getElementById(\"{identifier}\").focus(); 1;"),
        Duration::ZERO,
    )?;
    thread::sleep(Duration::from_millis(250));
    frames::page(application)
}

fn distributions(selected: &[&Value], phase: &str, metrics: &[(&str, &[&str])]) -> Result<Value> {
    let paints: (&str, &[&str]) = ("paints", &["computation", "paints"]);
    let mut values = Map::new();
    for (name, path) in metrics.iter().chain(std::iter::once(&paints)) {
        let samples = selected
            .iter()
            .map(|row| {
                path.iter()
                    .fold(&row["phases"][phase], |value, key| &value[*key])
                    .clone()
            })
            .collect::<Vec<_>>();
        values.insert((*name).into(), ipc::distribution(&samples)?);
    }
    Ok(Value::Object(values))
}

struct Host {
    seconds: f64,
    binary: PathBuf,
    project: PathBuf,
}

impl Host {
    fn capture(&self, directory: &Path, backend: &str, swapped: bool) -> Result<Value> {
        let mut application =
            Application::start(&self.binary, directory, backend, Some(&self.project))?;
        let captured = (|| {
            application.ready()?;
            let (mut current, _, _) = frames::wait(&application, None, None, Duration::ZERO)?;
            let order = if swapped {
                ["static", "animated"]
            } else {
                ["animated", "static"]
            };
            let mut samples = Map::new();
            for name in order {
                let identifier = if name == "animated" {
                    "task-input"
                } else {
                    "add"
                };
                let snapshot = frames::page(&application)?;
                let expected = snapshot["nodes"]
                    .as_array()
                    .and_then(|nodes| {
                        nodes
                            .iter()
                            .find(|node| node["attributes"]["id"] == identifier)
                    })
                    .and_then(|node| node["id"].as_i64())
                    .ok_or("focus target is absent")?;
                if snapshot["focused"].as_i64() != Some(expected) {
                    focus(&application, &current, identifier)?;
                }
                samples.insert(name.into(), phase(&application, self.seconds)?);
                current = frames::page(&application)?;
            }
            if samples["animated"]["computation"]["animations"]
                .as_i64()
                .unwrap_or(0)
                == 0
            {
                return Err("focused input did not produce animation samples".into());
            }
            if samples["static"]["computation"]["animations"]
                .as_i64()
                .unwrap_or(0)
                != 0
            {
                return Err("unfocused input continued producing animation samples".into());
            }
            for sample in samples.values() {
                if sample["computation"]["layouts"].as_i64().unwrap_or(0) != 0
                    || sample["computation"]["paints"].as_i64().unwrap_or(0) != 0
                {
                    return Err("visual-only activity reconstructed layout or paint".into());
                }
            }
            Ok(json!({"backend": backend, "order": order, "phases": samples}))
        })();
        let closed = application.close();
        let result = match (captured, closed) {
            (Ok(value), Ok(())) => value,
            (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
        };
        write(&directory.join("result.json"), &result)?;
        Ok(result)
    }
}
