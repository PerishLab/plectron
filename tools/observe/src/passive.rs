use crate::application::Application;
use crate::{Result, absolute, array, create_output, digest, frames, integer, ipc, read, write};
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

const MODES: [&str; 4] = ["zero", "low", "high", "wait"];
const QUIET: Duration = Duration::from_millis(150);
const INTERVAL: Duration = Duration::from_millis(5);

pub struct Config {
    pub binary: PathBuf,
    pub output: PathBuf,
    pub apps: PathBuf,
    pub rounds: usize,
    pub runs: usize,
}

pub fn durations(task: &Value, frame: &Value) -> Result<Value> {
    for item in [task, frame] {
        let admitted = integer(&item["admitted"], "admitted timestamp")?;
        let produced = integer(&item["produced"], "produced timestamp")?;
        let detected = integer(&item["detected"], "detected timestamp")?;
        if !(admitted <= produced && produced <= detected) {
            return Err(format!("invalid host timestamp topology: {item}"));
        }
    }
    Ok(json!({
        "owner_completion": integer(&frame["detected"], "frame detection")? - integer(&task["admitted"], "task admission")?,
        "task_production": integer(&task["produced"], "task production")? - integer(&task["admitted"], "task admission")?,
        "task_publication": integer(&task["detected"], "task detection")? - integer(&task["produced"], "task production")?,
        "frame_production": integer(&frame["produced"], "frame production")? - integer(&frame["admitted"], "frame admission")?,
        "frame_publication": integer(&frame["detected"], "frame detection")? - integer(&frame["produced"], "frame production")?,
    }))
}

pub fn run(config: &Config) -> Result<Value> {
    crate::application::require()?;
    if config.rounds == 0 || config.runs == 0 {
        return Err("rounds and runs must be positive".into());
    }
    let root = create_output(&config.output)?;
    let binary = absolute(&config.binary)?;
    let apps = absolute(&config.apps)?;
    let variants = frames::CASES
        .iter()
        .flat_map(|(case, _)| MODES.map(|mode| (*case, mode)))
        .collect::<Vec<_>>();
    let mut rows = Vec::new();
    for index in 0..config.rounds {
        let mut batch = BTreeMap::new();
        for offset in 0..variants.len() {
            let (case, mode) = variants[(index + offset) % variants.len()];
            eprintln!("[{}/{}] {case} {mode}", index + 1, config.rounds);
            let result = capture(
                config,
                &binary,
                &root.join(format!("{index}-{case}-{mode}")),
                &apps,
                case,
                mode,
            )?;
            rows.push(
                json!({"round": index, "case": case, "mode": mode, "timing": result["timing"]}),
            );
            batch.insert((case, mode), result);
        }
        for (case, _) in frames::CASES {
            for mode in MODES {
                frames::verify(&batch[&(case, "zero")], &batch[&(case, mode)])?;
            }
        }
    }
    let bytes = std::fs::read(&binary).map_err(|error| error.to_string())?;
    let result = json!({
        "binary": binary, "sha256": digest(&bytes), "rounds": config.rounds, "runs": config.runs,
        "warmup": frames::WARMUP, "unit": "nanoseconds", "zero_interval_seconds": QUIET.as_secs_f64(),
        "low_interval_seconds": INTERVAL.as_secs_f64(),
        "endpoint": "owner-published successful presentation receipt; host timestamps share publication-store epoch",
        "caveats": ["Frame production includes render, drawable acquisition and presentation-copy completion; not hardware GPU duration or scanout.", "Task publication includes owner processing after receipt; component distributions overlap and must not be summed.", "Zero mode performs no requests after submission until the quiet interval ends; setup and post-completion inspection are explicit.", "Client wall includes deliberate quiet or polling delay; semantic signatures are not pixel comparison."],
        "rows": rows,
    });
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}

fn query(application: &Application, command: Value) -> Result<Value> {
    Ok(application.request(&json!({"type": "query", "command": command}))?["reply"].clone())
}

fn record(reply: &Value) -> Result<&Value> {
    let status = &reply["status"];
    if status["type"] != "record" {
        return Err(format!("completion record unavailable: {reply}"));
    }
    let result = &status["record"];
    if result["state"]["type"] != "pending" && result["state"]["type"] != "complete" {
        return Err(format!("completion unsuccessful: {result}"));
    }
    if result["state"]["type"] == "complete" && result["state"]["result"].get("Ok").is_none() {
        return Err(format!("completion failed: {result}"));
    }
    Ok(result)
}

fn terminal(application: &Application, identity: i64, mode: &str) -> Result<(Value, usize, u128)> {
    let start = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut count = 0;
    while Instant::now() < deadline {
        let command = if mode == "wait" {
            json!({"type": "wait", "record": identity, "timeout": 2_000})
        } else {
            json!({"type": "read", "record": identity})
        };
        let reply = query(application, command)?;
        let current = record(&reply)?;
        count += 1;
        if current["state"]["type"] == "complete" {
            return Ok((current.clone(), count, start.elapsed().as_nanos()));
        }
        if mode == "zero" {
            return Err("execution did not complete during the query-free interval".into());
        }
        if mode == "low" {
            thread::sleep(INTERVAL);
        }
    }
    Err("passive completion timed out".into())
}

fn action(
    application: &Application,
    previous: &Value,
    source: &str,
    mode: &str,
) -> Result<(Value, Value)> {
    let start = Instant::now();
    let accepted =
        application.request(&json!({"type": "evaluate", "unit": 1, "source": source}))?;
    let admission = start.elapsed().as_nanos();
    if mode == "zero" {
        thread::sleep(QUIET);
    }
    let (task, queries, inquiry) = terminal(
        application,
        integer(&accepted["record"], "task record")?,
        mode,
    )?;
    let latest = query(application, json!({"type": "latest", "unit": 1}))?;
    let latest = record(&latest)?;
    if latest["target"]["frame"] == previous["frame"] {
        return Err("new task did not publish a new frame".into());
    }
    let (frame, count, elapsed) =
        terminal(application, integer(&latest["id"], "frame record")?, mode)?;
    let wall = start.elapsed().as_nanos();
    let snapshot = frames::page(application)?;
    if !frames::settled(&snapshot) || snapshot["frame"] != frame["target"]["frame"] {
        return Err("published frame differs from explicit page inspection".into());
    }
    let before = previous["computation"]
        .as_object()
        .ok_or("previous computation must be an object")?;
    let computation = snapshot["computation"]
        .as_object()
        .ok_or("computation must be an object")?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                json!(integer(value, key)? - before.get(key).and_then(Value::as_i64).unwrap_or(0)),
            ))
        })
        .collect::<Result<Map<_, _>>>()?;
    let mut sample = durations(&task, &frame)?.as_object().cloned().unwrap();
    sample.extend(Map::from_iter([
        ("task".into(), task),
        ("frame".into(), frame),
        ("queries".into(), json!(queries + count + 1)),
        ("wall".into(), json!(wall)),
        ("admission".into(), json!(admission)),
        ("inquiry".into(), json!(inquiry + elapsed)),
        ("computation".into(), json!(computation)),
        ("signature".into(), json!(frames::signature(&snapshot)?)),
    ]));
    Ok((snapshot, Value::Object(sample)))
}

fn capture(
    config: &Config,
    binary: &Path,
    directory: &Path,
    apps: &Path,
    case: &str,
    mode: &str,
) -> Result<Value> {
    let mut application = Application::start(binary, directory, "metal", None)?;
    let captured = (|| {
        application.ready()?;
        let root =
            serde_json::to_string(&apps.to_string_lossy()).map_err(|error| error.to_string())?;
        application.evaluate(&format!("const server = platform.serve({root}); server.scheme(\"app://passive/\"); const win = platform.window({{title:\"Passive frame receipt\",width:720,height:640}}); win.load(\"app://passive/{case}/index.html\"); 1;"))?;
        let (mut previous, _, _) = frames::wait(&application, None, None, Duration::ZERO)?;
        let catalog = read(&apps.join("catalog.json"))?;
        let size = array(&catalog, "catalog")?
            .iter()
            .find(|item| item["id"] == case)
            .and_then(|item| item["sizes"]["small"].as_i64())
            .ok_or("case size is absent")?;
        let ids = serde_json::to_string(&(0..size).collect::<Vec<_>>())
            .map_err(|error| error.to_string())?;
        (previous, _) = frames::action(
            &application,
            &previous,
            &format!("scenario.seed({ids}); 1;"),
            Duration::ZERO,
        )?;
        let mut samples = Vec::new();
        let action_name = frames::CASES
            .iter()
            .find(|(name, _)| *name == case)
            .unwrap()
            .1;
        for index in 0..config.runs + frames::WARMUP {
            let (next, sample) = action(
                &application,
                &previous,
                &format!("scenario.run(\"{action_name}\", []); 1;"),
                mode,
            )?;
            previous = next;
            if index >= frames::WARMUP {
                samples.push(sample);
            }
        }
        write(&directory.join("snapshot.json"), &previous)?;
        let timing = [
            "owner_completion",
            "task_production",
            "task_publication",
            "frame_production",
            "frame_publication",
            "wall",
            "queries",
        ]
        .into_iter()
        .map(|key| {
            Ok((
                key.into(),
                ipc::distribution(
                    &samples
                        .iter()
                        .map(|sample| sample[key].clone())
                        .collect::<Vec<_>>(),
                )?,
            ))
        })
        .collect::<Result<Map<_, _>>>()?;
        Ok(json!({"case": case, "mode": mode, "count": size, "samples": samples, "timing": timing}))
    })();
    let closed = application.close();
    let result = match (captured, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
    };
    write(&directory.join("result.json"), &result)?;
    Ok(result)
}
