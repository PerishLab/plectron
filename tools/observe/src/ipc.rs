use crate::application::Application;
use crate::bench;
use crate::{Result, absolute, array, create_output, digest, integer, number, report, write};
#[cfg(unix)]
use nix::sys::{
    resource::{UsageWho, getrusage},
    time::TimeValLike,
};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

pub const WARMUP: usize = 5;
const CASES: [(&str, &str, usize); 4] = [
    ("compute", "count = count + 1; 1;", 0),
    ("single", "console.log(1); count = count + 1; 1;", 1),
    (
        "burst",
        "for (const value of [1,1,1,1,1,1,1,1]) { console.log(value); } count = count + 1; 1;",
        8,
    ),
    (
        "promise",
        "Promise.resolve(1).then(value => { count = count + value; console.log(value); }); 1;",
        1,
    ),
];

pub struct Config {
    pub baseline: PathBuf,
    pub internal: PathBuf,
    pub candidate: PathBuf,
    pub output: PathBuf,
    pub rounds: usize,
    pub runs: usize,
}

pub fn distribution(values: &[Value]) -> Result<Value> {
    if values.is_empty() {
        return Err("distribution has no samples".into());
    }
    let mut values = values
        .iter()
        .map(|value| number(value, "sample"))
        .collect::<Result<Vec<_>>>()?;
    values.sort_by(f64::total_cmp);
    let count = values.len();
    Ok(json!({
        "median": bench::median(&values)?,
        "p95": values[(count * 95).div_ceil(100) - 1],
        "maximum": values[count - 1],
    }))
}

pub fn aggregate(batches: &[Value]) -> Result<Value> {
    let mut output = Vec::new();
    for traced in [false, true] {
        let mut paired = Vec::new();
        for batch in batches {
            let modes = batch[traced.to_string()]
                .as_object()
                .ok_or("IPC modes must be an object")?;
            let mut converted = serde_json::Map::new();
            for (mode, result) in modes {
                let entries = result["actions"].as_object().ok_or("IPC actions must be an object")?.iter().map(|(case, data)| {
                    let timing = ["wall", "admission"].into_iter().map(|metric| {
                        Ok((metric.into(), json!({"median": number(&data["timing"][metric]["median"], metric)? / 1_000_000.0})))
                    }).collect::<Result<serde_json::Map<_, _>>>()?;
                    Ok(json!({"id": case, "summary": {"unit": "milliseconds", "actions": {"execute": {"timing": timing}}}}))
                }).collect::<Result<Vec<_>>>()?;
                converted.insert(
                    if mode == "baseline" {
                        "off".into()
                    } else {
                        mode.clone()
                    },
                    Value::Array(entries),
                );
            }
            paired.push(Value::Object(converted));
        }
        let rows = bench::aggregate(&paired)?;
        for mut row in array(&rows, "IPC aggregate rows")?.iter().cloned() {
            row["traced"] = json!(traced);
            output.push(row);
        }
    }
    Ok(Value::Array(output))
}

pub fn run(config: &Config) -> Result<Value> {
    crate::application::require()?;
    if config.rounds == 0 || config.runs == 0 {
        return Err("rounds and runs must be positive".into());
    }
    let root = create_output(&config.output)?;
    let binaries = [
        ("baseline", absolute(&config.baseline)?),
        ("internal", absolute(&config.internal)?),
        ("candidate", absolute(&config.candidate)?),
    ];
    let variants = [false, true]
        .into_iter()
        .flat_map(|traced| binaries.iter().map(move |(mode, _)| (traced, *mode)))
        .collect::<Vec<_>>();
    let mut batches = Vec::new();
    for round in 0..config.rounds {
        let mut batch = Map::from_iter([("false".into(), json!({})), ("true".into(), json!({}))]);
        for offset in 0..variants.len() {
            let (traced, mode) = variants[(round + offset) % variants.len()];
            eprintln!("[{}/{}] {mode} traced={traced}", round + 1, config.rounds);
            let binary = binaries
                .iter()
                .find(|(name, _)| *name == mode)
                .unwrap()
                .1
                .as_path();
            let target = root.join(format!("{round}-{mode}-{}", usize::from(traced)));
            batch[&traced.to_string()][mode] = capture(binary, &target, config.runs, traced)?;
        }
        batches.push(Value::Object(batch));
    }
    let identities = binaries
        .iter()
        .map(|(name, path)| {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            Ok((
                (*name).into(),
                json!({"path": path, "sha256": digest(&bytes)}),
            ))
        })
        .collect::<Result<Map<_, _>>>()?;
    let result = json!({
        "unit": "milliseconds", "rounds": config.rounds, "runs": config.runs, "warmup": WARMUP,
        "binaries": identities,
        "endpoint": "Client socket connect through eval acknowledgement and first control reply confirming complete; includes repeated fresh connections and JSON decoding, no CLI process per request",
        "limits": ["Warm headless application, no render claim", "Client polls status without added sleep; poll counts recorded", "Traced captures include warmups, final state check and idle phase", "Action wall excludes startup, observation export and shutdown"],
        "comparison": aggregate(&batches)?,
    });
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}

fn capture(binary: &Path, directory: &Path, runs: usize, traced: bool) -> Result<Value> {
    let before = usage()?;
    let mut application = Application::start(binary, directory, "cpu", None)?;
    let captured = (|| {
        application.ready()?;
        let identity = if traced {
            Some(integer(
                &application.observation(&json!({"type": "start", "options": {"cpu": true}}))?["session"],
                "session",
            )?)
        } else {
            None
        };
        let started = Instant::now();
        thread::sleep(Duration::from_millis(500));
        let idle = application.request(&json!({"type": "status"}))?;
        let idle_wall = started.elapsed().as_nanos();
        if idle["mode"] != "running" {
            return Err(format!("idle application failed: {idle}"));
        }
        let mut records = Map::new();
        for (name, source, _) in CASES {
            let mut samples = Vec::new();
            for index in 0..runs + WARMUP {
                let sample = application.evaluate(source)?;
                if sample["result"] != "1" {
                    return Err(format!("unexpected script result: {sample}"));
                }
                if index >= WARMUP {
                    samples.push(sample);
                }
            }
            let timing = ["wall", "admission", "polls"]
                .into_iter()
                .map(|key| {
                    Ok((
                        key.into(),
                        distribution(
                            &samples
                                .iter()
                                .map(|value| value[key].clone())
                                .collect::<Vec<_>>(),
                        )?,
                    ))
                })
                .collect::<Result<Map<_, _>>>()?;
            records.insert(name.into(), json!({"samples": samples, "timing": timing}));
        }
        let expected = (runs + WARMUP) * CASES.len();
        if application.evaluate("count;")?["result"]
            .as_str()
            .and_then(|value| value.parse::<usize>().ok())
            != Some(expected)
        {
            return Err("script state did not match completed actions".into());
        }
        let diagnostics = if let Some(identity) = identity {
            let result =
                report::write(&application.export(identity)?, &directory.join("analysis"))?;
            if result["complete"].as_bool() != Some(true)
                || result["successful"].as_bool() != Some(true)
            {
                return Err("incomplete application observation".into());
            }
            result
        } else {
            Value::Null
        };
        Ok((records, diagnostics, idle_wall))
    })();
    let closed = application.close();
    let (records, diagnostics, idle_wall) = match (captured, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
    };
    let after = usage()?;
    let expected = (runs + WARMUP) * CASES.iter().map(|(_, _, lines)| lines).sum::<usize>();
    let output =
        std::fs::read_to_string(directory.join("stdout.log")).map_err(|error| error.to_string())?;
    if output.lines().collect::<Vec<_>>() != vec!["1"; expected] {
        return Err("platform log output mismatch".into());
    }
    let result = json!({
        "actions": records, "observation": diagnostics, "idle_wall": idle_wall,
        "children_cpu": {"user": after.0 - before.0, "system": after.1 - before.1},
        "cpu_endpoint": "Whole application lifecycle plus export helper in traced runs; includes startup and shutdown, not per-action CPU",
    });
    write(&directory.join("result.json"), &result)?;
    Ok(result)
}

#[cfg(unix)]
fn usage() -> Result<(f64, f64)> {
    let value = getrusage(UsageWho::RUSAGE_CHILDREN).map_err(|error| error.to_string())?;
    Ok((
        value.user_time().num_microseconds() as f64 / 1_000_000.0,
        value.system_time().num_microseconds() as f64 / 1_000_000.0,
    ))
}

#[cfg(not(unix))]
fn usage() -> Result<(f64, f64)> {
    Err("child CPU accounting requires Unix".into())
}
