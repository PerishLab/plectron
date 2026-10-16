use crate::application::Application;
use crate::{Result, array, digest, integer};
use serde_json::{Map, Value, json};
use std::thread;
use std::time::{Duration, Instant};

mod probe;

pub use probe::{Config, run};

pub const WARMUP: usize = 4;
pub const CASES: [(&str, &str); 2] = [("cards", "width"), ("layers", "fade")];

pub fn settled(snapshot: &Value) -> bool {
    let presented = !snapshot["scene"].is_null()
        && snapshot["displayed"] == snapshot["scene"]
        && snapshot["presented"] == snapshot["revision"];
    let idle = snapshot["issued"].is_null() && snapshot["pending"].is_null();
    snapshot["state"] == "ready" && presented && idle
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
    let mut inspection = 0;
    while Instant::now() < deadline {
        let start = Instant::now();
        let snapshot = page(application)?;
        inspection += start.elapsed().as_nanos();
        queries += 1;
        let mut ready = settled(&snapshot);
        if let Some(job) = job {
            let finished = complete(&snapshot, job)?;
            ready = ready && finished;
        }
        if let Some(previous) = previous {
            ready = ready
                && integer(&snapshot["revision"], "revision")?
                    > integer(&previous["revision"], "previous revision")?
                && snapshot["frame"] != previous["frame"];
        }
        if ready {
            return Ok((snapshot, queries, inspection));
        }
        if !interval.is_zero() {
            thread::sleep(interval);
        }
    }
    Err("new frame receipt timed out".into())
}

fn complete(snapshot: &Value, job: i64) -> Result<bool> {
    let result = snapshot["execution"]["jobs"]
        .as_array()
        .and_then(|jobs| jobs.iter().find(|entry| entry["id"].as_i64() == Some(job)))
        .ok_or("content job is absent")?;
    if result["phase"] != "complete" {
        return Ok(false);
    }
    if result["result"].get("Ok").is_none() {
        return Err(format!("content job failed: {result}"));
    }
    Ok(true)
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
