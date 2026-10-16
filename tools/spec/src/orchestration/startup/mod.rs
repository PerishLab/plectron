mod process;
mod summary;
mod window;

use super::{Arguments, Host, output, stats};
use crate::Result;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::Duration;

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

pub fn run(values: &[String], host: Host) -> Result<Value> {
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
    let study = Study {
        args: &args,
        destination: &destination,
        runs,
        host,
    };
    match application {
        Some(root) => study.applications(Path::new(root)),
        None => study.scenarios(),
    }
}

struct Study<'a> {
    args: &'a Arguments<'a>,
    destination: &'a Path,
    runs: usize,
    host: Host<'a>,
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

fn gather(samples: &[&Value], path: &[&str]) -> Result<Value> {
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
    std::path::absolute(path).map_err(|error| error.to_string())
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
