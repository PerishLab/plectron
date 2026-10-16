mod session;

use crate::application::Application;
use crate::bench::median;
use crate::{Output, Result, absolute, array, digest, frames, integer, number, write};
use serde_json::{Value, json};
use session::{Capture, Session};
use std::path::PathBuf;

#[derive(clap::Args)]
pub struct Config {
    #[arg(long)]
    pub binary: PathBuf,
    #[arg(long)]
    pub project: PathBuf,
    #[arg(long)]
    pub output: PathBuf,
    #[arg(long, default_value = "manual")]
    pub label: String,
    #[arg(long, value_parser = ["manual", "forward", "reversal"], default_value = "manual")]
    pub drive: String,
    #[arg(long, default_value_t = 20.0)]
    pub seconds: f64,
    #[arg(long, default_value_t = 2.0)]
    pub interval: f64,
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
    let span = if visible.len() > 1 {
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
        "visible_span": span,
        "timing": {
            "admission_to_production": distribution(&production)?,
            "admission_to_visibility": distribution(&visibility)?,
            "production_to_visibility": distribution(&presentation)?,
            "production_to_detection": distribution(&detection)?,
            "active_visibility_cadence": distribution(&cadence)?,
        },
    }))
}

pub struct Drive<'a> {
    pub mode: &'a str,
    pub seconds: f64,
}

pub fn assess(summary: &Value, ending: &Value, observation: &Value, drive: &Drive) -> Value {
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
    if ending["settled"].as_bool() != Some(true) {
        reasons.push("presentation did not settle after input");
    }
    if summary["frames"].as_i64().unwrap_or(0) == 0 {
        reasons.push("no changed frame was observed");
    }
    if drive.mode != "manual"
        && summary["visible_span"].as_f64().unwrap_or(0.0) < drive.seconds * 1_000_000_000.0 * 0.8
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
    let positive = |value: f64| value.is_finite() && value > 0.0;
    if !(positive(config.seconds) && positive(config.interval)) {
        return Err("seconds and interval must be positive".into());
    }
    if !["manual", "forward", "reversal"].contains(&config.drive.as_str()) {
        return Err("drive must be manual, forward or reversal".into());
    }
    let root = Output::prepare(&config.output)?;
    let binary = absolute(&config.binary)?;
    let project = absolute(&config.project)?;
    let mut application =
        Application::start(&binary, &root.join("application"), "metal", Some(&project))?;
    let mut session = None;
    let captured = application.ready().and_then(|()| {
        Session {
            application: &application,
        }
        .capture(config, &root, &mut session)
    });
    if captured.is_err()
        && let Some(identity) = session.take()
    {
        let _ = application.export(identity);
    }
    let closed = application.close();
    let Capture {
        records,
        evicted,
        range,
        snapshot,
        trace,
        startup,
    } = match (captured, closed) {
        (Ok(value), Ok(())) => value,
        (Err(error), _) | (Ok(_), Err(error)) => return Err(error),
    };
    write(&root.join("frames.json"), &Value::Array(records.clone()))?;
    let ending = json!({
        "state": snapshot["state"], "revision": snapshot["revision"], "frame": snapshot["frame"],
        "presented": snapshot["presented"], "displayed": snapshot["displayed"], "issued": snapshot["issued"],
        "pending": snapshot["pending"], "settled": frames::settled(&snapshot), "range": range,
    });
    let observation = json!({"complete": trace["complete"], "successful": trace["successful"], "dropped": trace["dropped"], "open": trace["open"]});
    let summary = summarize(&records, &evicted)?;
    let bytes = std::fs::read(&binary).map_err(|error| error.to_string())?;
    let result = json!({
        "version": 1, "unit": "nanoseconds", "label": config.label, "drive": config.drive,
        "seconds": config.seconds, "poll_interval_ms": config.interval,
        "startup": {"ready_to_settled": startup.elapsed, "queries": startup.queries, "inspection": startup.inspection},
        "binary": {"path": binary, "sha256": digest(&bytes)}, "project": project,
        "endpoint": "Metal drawable presentedTime with frame submission, backend completion and owner detection on the platform-lifetime monotonic clock",
        "limits": ["Physical presentation timestamp is not photon arrival or display scanout completion.", "Cadence includes adjacent visible frames only when their admissions are at most 100ms apart; input cadence can still dominate it.", "The query service is external to the application owner loop, but socket, query and observation overhead remain present.", "A manual run records one human gesture session; scripted driving calibrates the Display path without claiming native input behavior."],
        "assessment": assess(&summary, &ending, &observation, &Drive { mode: &config.drive, seconds: config.seconds }),
        "summary": summary, "final": ending, "observation": observation,
    });
    write(&root.join("summary.json"), &result)?;
    Ok(result)
}
