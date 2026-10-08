use crate::summary::summarize;
use crate::{Result, digest, read};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub const ACTIONS: [&str; 8] = [
    "seed", "steady", "append", "edit", "reorder", "remove", "width", "scroll",
];

pub fn identity(report: &Value) -> Result<Value> {
    let fields = [
        "version",
        "browser",
        "host",
        "sources",
        "reset",
        "adaptations",
        "differences",
        "endpoint",
        "accounting",
        "gpu",
    ];
    if fields.iter().any(|name| !truthy(&report[name])) {
        return Err(
            "reference requires complete browser, host, source and measurement identity".into(),
        );
    }
    if !truthy(&report["revision"]) {
        return Err("reference requires source revision".into());
    }
    let mut result: Map<_, _> = fields
        .iter()
        .map(|name| ((*name).to_owned(), report[name].clone()))
        .collect();
    result.insert(
        "workload".into(),
        json!({
            "count": report["config"]["count"],
            "warmups": report["config"]["warmups"],
        }),
    );
    for name in ["artifacts", "translator", "prerequisites"] {
        if let Some(value) = report.get(name) {
            result.insert(name.into(), value.clone());
        }
    }
    if let Some(attributes) = result["gpu"]["auxAttributes"].as_object_mut() {
        attributes.remove("initializationTime");
    }
    Ok(Value::Object(result))
}

pub fn validate(report: &Value) -> Result<Value> {
    if report["errors"]
        .as_array()
        .is_none_or(|errors| !errors.is_empty())
    {
        return Err("browser errors invalidate the reference".into());
    }
    let count = report["config"]["runs"]
        .as_u64()
        .ok_or_else(|| "reference runs must be an integer".to_owned())?;
    if count == 0 {
        return Err("reference requires measured repetitions".into());
    }
    let runs = report["runs"]
        .as_array()
        .ok_or_else(|| "runs must be an array".to_owned())?;
    let mut states = Map::new();
    for mode in ["plain", "trace"] {
        let selected: Vec<_> = runs.iter().filter(|run| run["mode"] == mode).collect();
        let mut indices: Vec<_> = selected
            .iter()
            .filter_map(|run| run["index"].as_u64())
            .collect();
        indices.sort_unstable();
        if indices != (0..count).collect::<Vec<_>>() {
            return Err("incomplete or duplicate repetitions".into());
        }
        for run in selected {
            let stages = run["stages"]
                .as_array()
                .ok_or_else(|| "stages must be an array".to_owned())?;
            let actions: Vec<_> = stages
                .iter()
                .filter_map(|stage| stage["action"].as_str())
                .collect();
            if actions != ACTIONS {
                return Err("incomplete or reordered actions".into());
            }
            for stage in stages {
                let action = stage["action"].as_str().unwrap_or_default();
                if let Some(previous) = states.get(action) {
                    if previous != &stage["state"] {
                        return Err("observed workload changed across repetitions".into());
                    }
                } else {
                    states.insert(action.to_owned(), stage["state"].clone());
                }
            }
        }
    }
    if runs.len() != count as usize * 2 {
        return Err("unexpected measurement mode".into());
    }
    Ok(Value::Object(states))
}

pub fn freeze(directories: &[PathBuf]) -> Result<Value> {
    let distinct: BTreeSet<_> = directories
        .iter()
        .map(|path| absolute(path))
        .collect::<Result<_>>()?;
    if distinct.len() != directories.len() {
        return Err("reference batches must be distinct".into());
    }
    let mut batches = Vec::new();
    let mut common = None;
    let mut observed = None;
    for directory in directories {
        let report = read(&directory.join("report.json"))?;
        let signature = identity(&report)?;
        let states = validate(&report)?;
        if common.as_ref().is_some_and(|held| held != &signature)
            || observed.as_ref().is_some_and(|held| held != &states)
        {
            return Err(
                "reference batches differ in environment, harness or observed workload".into(),
            );
        }
        common = Some(signature);
        observed = Some(states);
        let summary = summarize(directory)?;
        let artifacts: Map<_, _> = ["report.json", "trace.json", "summary.json"]
            .iter()
            .map(|name| {
                let path = directory.join(name);
                let data =
                    std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
                Ok(((*name).to_owned(), json!(digest(&data))))
            })
            .collect::<Result<_>>()?;
        batches.push(json!({
            "id": directory.file_name().and_then(|name| name.to_str()).unwrap_or_default(),
            "revision": report["revision"],
            "gpu_initialization_seconds": report["gpu"]["auxAttributes"]["initializationTime"],
            "config": {
                "count": report["config"]["count"],
                "runs": report["config"]["runs"],
                "warmups": report["config"]["warmups"],
            },
            "artifacts": artifacts,
            "actions": summary["actions"],
        }));
    }
    if batches.len() < 2 {
        return Err("reference requires at least two batches".into());
    }
    let mut actions = Map::new();
    for action in ACTIONS {
        let traces: Vec<_> = batches
            .iter()
            .map(|batch| &batch["actions"][action]["trace"])
            .collect();
        let phases = traces[0]["phases"]
            .as_object()
            .ok_or_else(|| "reference trace phases must be an object".to_owned())?;
        let ranges: Map<_, _> = phases
            .keys()
            .map(|name| {
                let values: Vec<_> = traces
                    .iter()
                    .filter_map(|trace| trace["phases"][name]["median"].as_f64())
                    .collect();
                Ok((name.clone(), range(&values)?))
            })
            .collect::<Result<_>>()?;
        actions.insert(
            action.into(),
            json!({
                "main_median_range": range(&traces.iter().filter_map(|trace| trace["trace"]["main"]["median"].as_f64()).collect::<Vec<_>>())?,
                "main_p95_range": range(&traces.iter().filter_map(|trace| trace["trace"]["main"]["p95"].as_f64()).collect::<Vec<_>>())?,
                "phase_median_ranges": ranges,
            }),
        );
    }
    for batch in &mut batches {
        batch.as_object_mut().unwrap().remove("actions");
    }
    Ok(json!({
        "schema": 1,
        "status": "provisional",
        "scenario": "cards",
        "unit": "milliseconds",
        "analysis_sources": {
            "reference.rs": digest(include_bytes!("reference.rs")),
            "summary.rs": digest(include_bytes!("summary.rs")),
        },
        "cross_engine_ratio": false,
        "identity": common,
        "states": observed,
        "actions": actions,
        "batches": batches,
        "interpretation": {
            "primary": "Union of main-thread RunTask wall intervals within action markers; not thread CPU time or end-to-end frame completion.",
            "ranges": "Range of batch medians/p95 values, not a confidence interval or acceptance threshold.",
            "comparison": "Directional phase/work reference only; margin geometry and rendering endpoints remain unmatched to Plectron.",
            "secondary": "RasterTask/GPUTask thread intervals can overlap and are not hardware GPU duration; do not add them to main.",
            "plain": "Untraced CDP metrics expose instrumentation sensitivity but TaskDuration includes harness/protocol work.",
            "startup": "Not measured by this reference; fresh browser/application startup requires a separate endpoint contract.",
        },
    }))
}

fn range(values: &[f64]) -> Result<Value> {
    let minimum = values
        .iter()
        .copied()
        .reduce(f64::min)
        .ok_or_else(|| "range has no samples".to_owned())?;
    let maximum = values.iter().copied().reduce(f64::max).unwrap();
    Ok(json!([minimum, maximum]))
}

fn absolute(path: &Path) -> Result<PathBuf> {
    path.canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}
