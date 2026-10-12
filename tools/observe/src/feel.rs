use crate::application::Application;
use crate::bench::median;
use crate::{
    Result, absolute, array, create_output, digest, frames, integer, number, report, write,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

pub struct Config {
    pub binary: PathBuf,
    pub project: PathBuf,
    pub output: PathBuf,
    pub label: String,
    pub drive: String,
    pub seconds: f64,
    pub interval_ms: f64,
}

pub fn distribution(values: &[Value]) -> Result<Value> {
    if values.is_empty() {
        return Ok(Value::Null);
    }
    let mut values = values
        .iter()
        .map(|value| number(value, "sample"))
        .collect::<Result<Vec<_>>>()?;
    values.sort_by(f64::total_cmp);
    let count = values.len();
    Ok(json!({
        "median": median(&values)?,
        "p95": values[(count * 95).div_ceil(100) - 1],
        "p99": values[(count * 99).div_ceil(100) - 1],
        "maximum": values[count - 1],
    }))
}

pub fn summarize(records: &[Value], evicted: &[Value]) -> Result<Value> {
    let complete = records
        .iter()
        .filter(|record| record["state"]["type"] == "complete")
        .collect::<Vec<_>>();
    let successful = complete
        .iter()
        .copied()
        .filter(|record| record["state"]["result"].get("Ok").is_some())
        .collect::<Vec<_>>();
    let visible = successful
        .iter()
        .copied()
        .filter(|record| !record["visible"].is_null())
        .collect::<Vec<_>>();
    let differences = |left: &str, right: &str, values: &[&Value]| -> Vec<Value> {
        values
            .iter()
            .filter_map(|record| Some(json!(record[left].as_i64()? - record[right].as_i64()?)))
            .collect()
    };
    let production = differences("produced", "admitted", &successful);
    let visibility = differences("visible", "admitted", &visible);
    let presentation = visible
        .iter()
        .filter_map(|record| {
            Some(json!(
                record["visible"].as_i64()? - record["produced"].as_i64()?
            ))
        })
        .collect::<Vec<_>>();
    let detection = successful
        .iter()
        .filter_map(|record| {
            Some(json!(
                record["detected"].as_i64()? - record["produced"].as_i64()?
            ))
        })
        .collect::<Vec<_>>();
    let cadence = visible
        .windows(2)
        .filter_map(|pair| {
            let left = pair[0];
            let right = pair[1];
            let gap = right["admitted"].as_i64()? - left["admitted"].as_i64()?;
            let cadence = right["visible"].as_i64()? - left["visible"].as_i64()?;
            (gap <= 100_000_000).then(|| json!(cadence))
        })
        .collect::<Vec<_>>();
    let visible_span = if visible.len() > 1 {
        Some(
            integer(&visible[visible.len() - 1]["visible"], "visible timestamp")?
                - integer(&visible[0]["visible"], "visible timestamp")?,
        )
    } else {
        None
    };
    let drops = records
        .iter()
        .map(|record| record["drops"].as_i64().unwrap_or(0))
        .collect::<Vec<_>>();
    Ok(json!({
        "frames": records.len(),
        "complete": complete.len(),
        "successful": successful.len(),
        "visible": visible.len(),
        "missing_visibility": successful.len() - visible.len(),
        "evicted": evicted,
        "drops": {"total": drops.iter().sum::<i64>(), "maximum": drops.into_iter().max().unwrap_or(0)},
        "visible_span": visible_span,
        "timing": {
            "admission_to_production": distribution(&production)?,
            "admission_to_visibility": distribution(&visibility)?,
            "production_to_visibility": distribution(&presentation)?,
            "production_to_detection": distribution(&detection)?,
            "active_visibility_cadence": distribution(&cadence)?,
        },
    }))
}

pub fn assess(
    summary: &Value,
    final_state: &Value,
    observation: &Value,
    drive: &str,
    seconds: f64,
) -> Value {
    let mut reasons = Vec::new();
    let mut findings = Vec::new();
    if observation["complete"].as_bool() != Some(true)
        || observation["successful"].as_bool() != Some(true)
    {
        reasons.push("observation is incomplete or unsuccessful");
    }
    if array(&summary["evicted"], "evicted").is_ok_and(|values| !values.is_empty()) {
        reasons.push("frame records were evicted");
    }
    if summary["missing_visibility"].as_i64().unwrap_or(0) != 0 {
        reasons.push("backend-complete frames lack physical visibility receipts");
    }
    if summary["drops"]["total"].as_i64().unwrap_or(0) != 0 {
        findings.push("presentation reported dropped frames");
    }
    if final_state["settled"].as_bool() != Some(true) {
        reasons.push("presentation did not settle after input");
    }
    if summary["frames"].as_i64().unwrap_or(0) == 0 {
        reasons.push("no changed frame was observed");
    }
    if drive != "manual"
        && summary["visible_span"].as_f64().unwrap_or(0.0) < seconds * 1_000_000_000.0 * 0.8
    {
        reasons.push("physical visibility covers less than 80% of scripted input");
    }
    json!({"valid": reasons.is_empty(), "reasons": reasons, "findings": findings})
}

pub fn run(config: &Config) -> Result<Value> {
    crate::application::require()?;
    if !cfg!(target_os = "macos") {
        return Err("feel recording requires macOS physical presentation evidence".into());
    }
    if !config.seconds.is_finite()
        || !config.interval_ms.is_finite()
        || config.seconds <= 0.0
        || config.interval_ms <= 0.0
    {
        return Err("seconds and interval must be positive".into());
    }
    if !["manual", "forward", "reversal"].contains(&config.drive.as_str()) {
        return Err("drive must be manual, forward or reversal".into());
    }
    let root = create_output(&config.output)?;
    let binary = absolute(&config.binary)?;
    let project = absolute(&config.project)?;
    let mut application =
        Application::start(&binary, &root.join("application"), "metal", Some(&project))?;
    let mut session = None;
    let captured = (|| {
        application.ready()?;
        let (previous, startup, startup_queries, startup_inspection) = initial(&application)?;
        let (previous, _) = frames::action(
            &application,
            &previous,
            "scenario.start(); 1;",
            Duration::ZERO,
        )?;
        let _ = previous;
        let identity = integer(
            &application.observation(&json!({"type": "start", "options": {}}))?["session"],
            "session",
        )?;
        session = Some(identity);
        let action = if config.drive == "manual" {
            "interact with the Plectron window now".into()
        } else {
            format!("using {} display-path input", config.drive)
        };
        eprintln!(
            "feel: recording {:?} for {}s; {action}",
            config.label, config.seconds
        );
        let (mut records, evicted) = collect(
            &application,
            config.seconds,
            Duration::from_secs_f64(config.interval_ms / 1_000.0),
            &config.drive,
        )?;
        let final_range = evaluate(&application, "scenario.range();")?;
        let snapshot = drain(&application, Duration::from_secs(2))?;
        refresh(&application, &mut records)?;
        let trace = report::write(&application.export(identity)?, &root.join("analysis"))?;
        session = None;
        Ok((
            records,
            evicted,
            final_range,
            snapshot,
            trace,
            startup,
            startup_queries,
            startup_inspection,
        ))
    })();
    if captured.is_err()
        && let Some(identity) = session.take()
    {
        let _ = application.export(identity);
    }
    let closed = application.close();
    let (
        records,
        evicted,
        final_range,
        snapshot,
        trace,
        startup,
        startup_queries,
        startup_inspection,
    ) = match (captured, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
    };
    write(&root.join("frames.json"), &Value::Array(records.clone()))?;
    let final_state = json!({
        "state": snapshot["state"], "revision": snapshot["revision"], "frame": snapshot["frame"],
        "presented": snapshot["presented"], "displayed": snapshot["displayed"], "issued": snapshot["issued"],
        "pending": snapshot["pending"], "settled": frames::settled(&snapshot), "range": final_range,
    });
    let observation = json!({"complete": trace["complete"], "successful": trace["successful"], "dropped": trace["dropped"], "open": trace["open"]});
    let summary = summarize(&records, &evicted)?;
    let bytes = std::fs::read(&binary).map_err(|error| error.to_string())?;
    let result = json!({
        "version": 1, "unit": "nanoseconds", "label": config.label, "drive": config.drive,
        "seconds": config.seconds, "poll_interval_ms": config.interval_ms,
        "startup": {"ready_to_settled": startup, "queries": startup_queries, "inspection": startup_inspection},
        "binary": {"path": binary, "sha256": digest(&bytes)}, "project": project,
        "endpoint": "Metal drawable presentedTime with frame submission, backend completion and owner detection on the platform-lifetime monotonic clock",
        "limits": ["Physical presentation timestamp is not photon arrival or display scanout completion.", "Cadence includes adjacent visible frames only when their admissions are at most 100ms apart; input cadence can still dominate it.", "The query service is external to the application owner loop, but socket, query and observation overhead remain present.", "A manual run records one human gesture session; scripted driving calibrates the Display path without claiming native input behavior."],
        "assessment": assess(&summary, &final_state, &observation, &config.drive, config.seconds),
        "summary": summary, "final": final_state, "observation": observation,
    });
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}

fn query(application: &Application, command: Value) -> Result<Value> {
    Ok(application.request(&json!({"type": "query", "command": command}))?["reply"].clone())
}

fn selected(reply: &Value) -> Option<&Value> {
    (reply["status"]["type"] == "record").then(|| &reply["status"]["record"])
}

fn evaluate(application: &Application, source: &str) -> Result<Value> {
    let result = application.request(&json!({"type": "evaluate", "unit": 1, "source": source}))?;
    if result["type"] == "evaluation" {
        return Ok(result["display"].clone());
    }
    let identity = integer(&result["job"], "job")?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let snapshot = frames::page(application)?;
        let job = snapshot["execution"]["jobs"]
            .as_array()
            .and_then(|jobs| jobs.iter().find(|job| job["id"].as_i64() == Some(identity)))
            .ok_or("content job is absent")?;
        if job["phase"] == "complete" {
            return job["result"]
                .get("Ok")
                .cloned()
                .ok_or_else(|| format!("content evaluation failed: {job}"));
        }
        thread::sleep(Duration::from_millis(2));
    }
    Err("content evaluation timed out".into())
}

fn initial(application: &Application) -> Result<(Value, u128, usize, u128)> {
    let started = Instant::now();
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        match frames::wait(application, None, None, Duration::from_millis(2)) {
            Ok((snapshot, queries, inspection)) => {
                return Ok((snapshot, started.elapsed().as_nanos(), queries, inspection));
            }
            Err(error)
                if error.contains("request handler did not reply") && Instant::now() < deadline => {
            }
            Err(error) => return Err(error),
        }
    }
}

fn collect(
    application: &Application,
    seconds: f64,
    interval: Duration,
    drive: &str,
) -> Result<(Vec<Value>, Vec<Value>)> {
    let reply = query(application, json!({"type": "latest", "unit": 1}))?;
    let initial = selected(&reply).ok_or("initial frame record is absent")?;
    let mut cursor = integer(&initial["id"], "initial record")?;
    let mut records = BTreeMap::new();
    let mut evicted = Vec::new();
    let deadline = Instant::now() + Duration::from_secs_f64(seconds);
    let mut next_drive = Instant::now();
    let midpoint = Instant::now() + Duration::from_secs_f64(seconds / 2.0);
    while Instant::now() < deadline {
        let now = Instant::now();
        if drive != "manual" && now >= next_drive {
            let direction = if drive == "reversal" && now >= midpoint {
                -1
            } else {
                1
            };
            application.request(
                &json!({"type": "wheel", "unit": 1, "x": 360, "y": 360, "delta": direction * 80}),
            )?;
            next_drive += Duration::from_secs_f64(1.0 / 60.0);
        }
        let reply = query(application, json!({"type": "latest", "unit": 1}))?;
        if let Some(latest) = selected(&reply) {
            let latest_id = integer(&latest["id"], "latest record")?;
            for identity in cursor + 1..=latest_id {
                let reply = query(application, json!({"type": "read", "record": identity}))?;
                if let Some(current) = selected(&reply) {
                    if current["target"]["type"] == "frame" && current["target"]["unit"] == 1 {
                        records.insert(identity, current.clone());
                    }
                } else if reply["status"]["type"] == "evicted" {
                    evicted.push(json!(identity));
                }
            }
            cursor = cursor.max(latest_id);
        }
        for (identity, value) in &mut records {
            if value["visible"].is_null() {
                let reply = query(application, json!({"type": "read", "record": identity}))?;
                if let Some(current) = selected(&reply) {
                    *value = current.clone();
                }
            }
        }
        let delay = if drive == "manual" {
            interval
        } else {
            interval.min(next_drive.saturating_duration_since(Instant::now()))
        };
        thread::sleep(delay);
    }
    for _ in 0..20 {
        let pending = records
            .iter()
            .filter(|(_, value)| value["visible"].is_null())
            .map(|(identity, _)| *identity)
            .collect::<Vec<_>>();
        if pending.is_empty() {
            break;
        }
        for identity in pending {
            let reply = query(application, json!({"type": "read", "record": identity}))?;
            if let Some(current) = selected(&reply) {
                records.insert(identity, current.clone());
            }
        }
        thread::sleep(interval);
    }
    evicted.sort_by_key(Value::as_i64);
    evicted.dedup();
    Ok((records.into_values().collect(), evicted))
}

fn refresh(application: &Application, records: &mut [Value]) -> Result<()> {
    for record in records {
        if record["state"]["type"] != "complete" || record["visible"].is_null() {
            let reply = query(application, json!({"type": "read", "record": record["id"]}))?;
            if let Some(current) = selected(&reply) {
                *record = current.clone();
            }
        }
    }
    Ok(())
}

fn drain(application: &Application, duration: Duration) -> Result<Value> {
    let deadline = Instant::now() + duration;
    let mut snapshot = frames::page(application)?;
    while !frames::settled(&snapshot) && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(2));
        snapshot = frames::page(application)?;
    }
    Ok(snapshot)
}
