use crate::application::Application;
use crate::bench::median;
use crate::{Result, absolute, array, digest, integer, ipc, read, report, write};
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

pub const WARMUP: usize = 4;
pub const CASES: [(&str, &str); 2] = [("cards", "width"), ("layers", "fade")];

pub fn settled(snapshot: &Value) -> bool {
    snapshot["state"] == "ready"
        && !snapshot["scene"].is_null()
        && snapshot["displayed"] == snapshot["scene"]
        && snapshot["presented"] == snapshot["revision"]
        && snapshot["issued"].is_null()
        && snapshot["pending"].is_null()
}

pub fn signature(snapshot: &Value) -> Result<String> {
    let shape: Map<_, _> = ["nodes", "visible", "visual", "shown", "items"]
        .into_iter()
        .map(|key| (key.into(), snapshot[key].clone()))
        .collect();
    serde_json::to_vec(&shape)
        .map(|data| digest(&data))
        .map_err(|error| error.to_string())
}

pub fn verify(left: &Value, right: &Value) -> Result<()> {
    let left = array(&left["samples"], "left samples")?;
    let right = array(&right["samples"], "right samples")?;
    let signatures = |values: &[Value]| {
        values
            .iter()
            .map(|sample| sample["signature"].clone())
            .collect::<Vec<_>>()
    };
    if signatures(left) != signatures(right) {
        return Err("frame semantic signatures differ".into());
    }
    let counts = ["layouts", "paints", "checks", "changes", "queries"];
    let work = |values: &[Value]| {
        values
            .iter()
            .map(|sample| counts.map(|key| sample["computation"][key].clone()))
            .collect::<Vec<_>>()
    };
    if work(left) != work(right) {
        return Err("frame computation counts differ".into());
    }
    Ok(())
}

pub fn page(application: &Application) -> Result<Value> {
    let snapshot = application.request(&json!({"type": "page", "unit": 1}))?["snapshot"].clone();
    if !snapshot["fault"].is_null()
        || !snapshot["output"].is_null()
        || snapshot["state"] == "failed"
    {
        return Err(format!(
            "page failed: {} {}",
            snapshot["fault"], snapshot["output"]
        ));
    }
    if !snapshot["execution"].is_null() && !snapshot["execution"]["fault"].is_null() {
        return Err(snapshot["execution"]["fault"].to_string());
    }
    Ok(snapshot)
}

pub fn wait(
    application: &Application,
    previous: Option<&Value>,
    job: Option<i64>,
    interval: Duration,
) -> Result<(Value, usize, u128)> {
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut queries = 0;
    let mut query_time = 0;
    while Instant::now() < deadline {
        let start = Instant::now();
        let snapshot = page(application)?;
        query_time += start.elapsed().as_nanos();
        queries += 1;
        let mut ready = settled(&snapshot);
        if let Some(job) = job {
            let result = snapshot["execution"]["jobs"]
                .as_array()
                .and_then(|jobs| jobs.iter().find(|entry| entry["id"].as_i64() == Some(job)))
                .ok_or("content job is absent")?;
            if result["phase"] == "complete" {
                if result["result"].get("Ok").is_none() {
                    return Err(format!("content job failed: {result}"));
                }
            } else {
                ready = false;
            }
        }
        if let Some(previous) = previous {
            ready = ready
                && integer(&snapshot["revision"], "revision")?
                    > integer(&previous["revision"], "previous revision")?
                && snapshot["frame"] != previous["frame"];
        }
        if ready {
            return Ok((snapshot, queries, query_time));
        }
        if !interval.is_zero() {
            thread::sleep(interval);
        }
    }
    Err("new frame receipt timed out".into())
}

pub fn action(
    application: &Application,
    previous: &Value,
    source: &str,
    interval: Duration,
) -> Result<(Value, Value)> {
    let start = Instant::now();
    let accepted =
        application.request(&json!({"type": "evaluate", "unit": 1, "source": source}))?;
    let admission = start.elapsed().as_nanos();
    let (snapshot, queries, inspection) = wait(
        application,
        Some(previous),
        accepted["job"].as_i64(),
        interval,
    )?;
    let before = previous["computation"]
        .as_object()
        .ok_or("previous computation must be an object")?;
    let after = snapshot["computation"]
        .as_object()
        .ok_or("computation must be an object")?;
    let computation: Map<_, _> = after
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                json!(integer(value, key)? - before.get(key).and_then(Value::as_i64).unwrap_or(0)),
            ))
        })
        .collect::<Result<_>>()?;
    let sample = json!({
        "wall": start.elapsed().as_nanos(),
        "admission": admission,
        "inspection": inspection,
        "queries": queries,
        "computation": computation,
        "signature": signature(&snapshot)?,
        "frame": snapshot["frame"],
        "revision": snapshot["revision"],
        "displayed": snapshot["displayed"],
    });
    Ok((snapshot, sample))
}

pub struct Config {
    pub baseline: PathBuf,
    pub candidate: PathBuf,
    pub output: PathBuf,
    pub apps: PathBuf,
    pub rounds: usize,
    pub runs: usize,
    pub interval_ms: f64,
}

pub fn run(config: &Config) -> Result<Value> {
    if config.runs < 1
        || config.rounds < 1
        || !config.interval_ms.is_finite()
        || config.interval_ms < 0.0
    {
        return Err("runs and rounds must be positive; interval must be nonnegative".into());
    }
    let root = output(&config.output)?;
    let apps = absolute(&config.apps)?;
    let binaries = [
        ("baseline", absolute(&config.baseline)?),
        ("candidate", absolute(&config.candidate)?),
    ];
    let mut variants = Vec::new();
    for case in CASES {
        for traced in [false, true] {
            for (mode, _) in &binaries {
                variants.push((case, traced, *mode));
            }
        }
    }
    let mut batches: Vec<BTreeBatch> = Vec::new();
    for index in 0..config.rounds {
        let mut batch = BTreeBatch::new();
        for offset in 0..variants.len() {
            let ((case, action), traced, mode) = variants[(index + offset) % variants.len()];
            eprintln!(
                "[{}/{}] {case} {mode} traced={traced}",
                index + 1,
                config.rounds
            );
            let binary = binaries
                .iter()
                .find(|(name, _)| *name == mode)
                .unwrap()
                .1
                .as_path();
            let target = root.join(format!("{index}-{case}-{mode}-{}", usize::from(traced)));
            batch.insert(
                (case.into(), traced, mode.into()),
                capture(config, binary, &target, &apps, case, action, traced)?,
            );
        }
        for (case, _) in CASES {
            let reference = &batch[&(case.into(), false, "baseline".into())];
            for traced in [false, true] {
                for (mode, _) in &binaries {
                    verify(reference, &batch[&(case.into(), traced, (*mode).into())])?;
                }
            }
        }
        batches.push(batch);
    }
    let comparison = compare(&batches)?;
    let identities: Map<_, _> = binaries
        .iter()
        .map(|(name, path)| {
            let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
            Ok((
                (*name).into(),
                json!({"path": path, "sha256": digest(&bytes)}),
            ))
        })
        .collect::<Result<_>>()?;
    let result = json!({
        "unit": "milliseconds", "rounds": config.rounds, "runs": config.runs, "warmup": WARMUP,
        "client_interval_ms": config.interval_ms, "binaries": identities,
        "endpoint": "Client eval connection through page response confirming completed content job and changed frame receipt: presented revision matches desired revision, displayed scene matches desired scene, no pending or issued frame",
        "limits": ["Metal completion and drawable-copy receipt is not physical scanout", "Full page inspection and JSON processing are included and timed separately; inspection can wake the main loop", "Small catalog scenes, six records each, 720x640 logical viewport", "No per-action hardware GPU duration exposed by this protocol", "Semantic scene/geometry hashes and work counters compared, not screenshots or pixel hashes", "Content/render/Metal algorithms are unchanged by the compared commits"],
        "comparison": comparison,
    });
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}

type BTreeBatch = std::collections::BTreeMap<(String, bool, String), Value>;

fn capture(
    config: &Config,
    binary: &Path,
    directory: &Path,
    apps: &Path,
    case: &str,
    action_name: &str,
    traced: bool,
) -> Result<Value> {
    let mut application = Application::start(binary, directory, "metal", None)?;
    let captured = (|| {
        application.ready()?;
        let root =
            serde_json::to_string(&apps.to_string_lossy()).map_err(|error| error.to_string())?;
        let source = format!(
            "const server = platform.serve({root}); server.scheme(\"app://frames/\"); const win = platform.window({{title: \"Frame receipt probe\", width: 720, height: 640}}); win.load(\"app://frames/{case}/index.html\"); 1;"
        );
        if application.evaluate(&source)?["result"] != "1" {
            return Err("launcher failed".into());
        }
        let (mut previous, _, _) = wait(&application, None, None, Duration::ZERO)?;
        let catalog = read(&apps.join("catalog.json"))?;
        let expected = array(&catalog, "catalog")?
            .iter()
            .find(|item| item["id"] == case)
            .and_then(|item| item["sizes"]["small"].as_i64())
            .ok_or("case size is absent")?;
        let ids = serde_json::to_string(&(0..expected).collect::<Vec<_>>())
            .map_err(|error| error.to_string())?;
        (previous, _) = action(
            &application,
            &previous,
            &format!("scenario.seed({ids}); 1;"),
            Duration::ZERO,
        )?;
        let identity = if traced {
            Some(integer(
                &application.observation(&json!({"type": "start", "options": {"cpu": true}}))?["session"],
                "session",
            )?)
        } else {
            None
        };
        let mut samples = Vec::new();
        for index in 0..config.runs + WARMUP {
            let (next, sample) = action(
                &application,
                &previous,
                &format!("scenario.run(\"{action_name}\", []); 1;"),
                Duration::from_secs_f64(config.interval_ms / 1_000.0),
            )?;
            previous = next;
            if index >= WARMUP {
                samples.push(sample);
            }
        }
        write(&directory.join("snapshot.json"), &previous)?;
        if let Some(identity) = identity {
            let summary =
                report::write(&application.export(identity)?, &directory.join("analysis"))?;
            if summary["complete"].as_bool() != Some(true)
                || summary["successful"].as_bool() != Some(true)
            {
                return Err("incomplete frame observation".into());
            }
        }
        let timing: Map<_, _> = ["wall", "admission", "inspection", "queries"]
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
            .collect::<Result<_>>()?;
        let computation = previous["computation"]
            .as_object()
            .ok_or("computation must be an object")?
            .keys()
            .map(|key| {
                Ok((
                    key.clone(),
                    ipc::distribution(
                        &samples
                            .iter()
                            .map(|sample| sample["computation"][key].clone())
                            .collect::<Vec<_>>(),
                    )?,
                ))
            })
            .collect::<Result<Map<_, _>>>()?;
        Ok(
            json!({"case": case, "action": action_name, "count": expected, "traced": traced, "samples": samples, "timing": timing, "computation": computation}),
        )
    })();
    let closed = application.close();
    let result = match (captured, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
    };
    write(&directory.join("result.json"), &result)?;
    Ok(result)
}

fn compare(batches: &[BTreeBatch]) -> Result<Value> {
    let mut rows = Vec::new();
    for (case, _) in CASES {
        for traced in [false, true] {
            for metric in ["wall", "admission", "inspection"] {
                let baseline = batches
                    .iter()
                    .map(|batch| {
                        batch[&(case.into(), traced, "baseline".into())]["timing"][metric]["median"]
                            .as_f64()
                            .unwrap_or(f64::NAN)
                            / 1_000_000.0
                    })
                    .collect::<Vec<_>>();
                let candidate = batches.iter().map(|batch| batch[&(case.into(), traced, "candidate".into())]["timing"][metric]["median"].as_f64().unwrap_or(f64::NAN) / 1_000_000.0).collect::<Vec<_>>();
                let ratios = baseline
                    .iter()
                    .zip(&candidate)
                    .map(|(base, value)| (value / base - 1.0) * 100.0)
                    .collect::<Vec<_>>();
                rows.push(json!({"case": case, "traced": traced, "metric": metric, "baseline": baseline, "candidate": candidate, "paired_percent": median(&ratios)?}));
            }
        }
    }
    Ok(Value::Array(rows))
}

fn output(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.into()
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    crate::create(&path)?;
    Ok(path)
}
