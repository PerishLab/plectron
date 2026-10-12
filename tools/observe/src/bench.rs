use crate::{
    Result, absolute, array, create_output, native, number, read, report, write, write_new,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

pub struct Config {
    pub plan: PathBuf,
    pub output: PathBuf,
    pub binary: PathBuf,
    pub seconds: u64,
    pub limit: Option<u64>,
    pub rounds: usize,
    pub runs: Option<usize>,
    pub modes: Vec<String>,
}

pub fn aggregate(batches: &[Value]) -> Result<Value> {
    let mut groups: BTreeMap<(String, String, String), BTreeMap<String, Vec<f64>>> =
        BTreeMap::new();
    for batch in batches {
        let modes = batch
            .as_object()
            .ok_or("benchmark batch must be an object")?;
        for (mode, entries) in modes {
            for entry in array(entries, "benchmark entries")? {
                let summary = &entry["summary"];
                if summary["unit"] != "milliseconds" {
                    return Err("unexpected summary unit".into());
                }
                let actions = summary["actions"]
                    .as_object()
                    .ok_or("actions must be an object")?;
                for (action, result) in actions {
                    let timing = result["timing"]
                        .as_object()
                        .ok_or("timing must be an object")?;
                    for (metric, value) in timing {
                        if !value.is_null() {
                            groups
                                .entry((text(entry, "id")?.into(), action.clone(), metric.clone()))
                                .or_default()
                                .entry(mode.clone())
                                .or_default()
                                .push(number(&value["median"], "timing median")?);
                        }
                    }
                }
            }
        }
    }
    let mut output = Vec::new();
    for ((entry, action, metric), modes) in groups {
        let baseline = modes.get("off").cloned().unwrap_or_default();
        for (mode, values) in modes {
            if values.len() != baseline.len() {
                return Err("unpaired observation batches".into());
            }
            let deltas = values
                .iter()
                .zip(&baseline)
                .map(|(value, base)| value - base)
                .collect::<Vec<_>>();
            let ratios = values
                .iter()
                .zip(&baseline)
                .filter(|(_, base)| **base > 0.0)
                .map(|(value, base)| (value / base - 1.0) * 100.0)
                .collect::<Vec<_>>();
            output.push(json!({
                "entry": entry,
                "action": action,
                "metric": metric,
                "mode": mode,
                "batches": values.len(),
                "medians": values,
                "median": median(&values)?,
                "paired_delta": median(&deltas)?,
                "paired_percent": (!ratios.is_empty()).then(|| median(&ratios)).transpose()?,
            }));
        }
    }
    Ok(Value::Array(output))
}

pub fn median(values: &[f64]) -> Result<f64> {
    if values.is_empty() {
        return Err("distribution has no samples".into());
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Ok(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

pub fn run(config: &Config) -> Result<Value> {
    if config.rounds == 0 || config.runs == Some(0) {
        return Err("rounds and runs must be positive".into());
    }
    let mut unique = config.modes.clone();
    unique.sort();
    unique.dedup();
    if !config.modes.iter().any(|mode| mode == "off") || unique.len() != config.modes.len() {
        return Err("modes require off and must be unique".into());
    }
    if config
        .modes
        .iter()
        .any(|mode| !["off", "wall", "cpu", "native"].contains(&mode.as_str()))
    {
        return Err("unsupported benchmark mode".into());
    }
    if config.modes.iter().any(|mode| mode == "native") {
        native::prerequisite()?;
    }
    let root = create_output(&config.output)?;
    let plan_path = absolute(&config.plan)?;
    let mut plan = read(&plan_path)?;
    let entries = plan["entries"]
        .as_array_mut()
        .ok_or("plan entries must be an array")?;
    for entry in entries {
        if let Some(baseline) = entry["baseline"].as_str() {
            let parent = plan_path.parent().ok_or("plan has no parent")?;
            entry["baseline"] = json!(absolute(&parent.join(baseline))?);
        }
        if let Some(runs) = config.runs {
            entry["runs"] = json!(runs);
        }
    }
    let source = root.join("plan.json");
    write_new(&source, &plan)?;
    let rules = root.join("rules.json");
    write_new(&rules, &json!({"same": ["images"], "vary": []}))?;
    let binary = absolute(&config.binary)?;
    let mut batches = Vec::new();
    let mut observations = Vec::new();
    for index in 0..config.rounds {
        let mut batch = serde_json::Map::new();
        let mut paths = BTreeMap::new();
        for offset in 0..config.modes.len() {
            let mode = &config.modes[(index + offset) % config.modes.len()];
            let directory = root.join(format!("{index}-{mode}"));
            eprintln!("[{}/{}] {mode}", index + 1, config.rounds);
            let started = Instant::now();
            let capture = execute(config, &source, &directory, mode, &binary)?;
            let target = if mode == "native" {
                directory.join("scenario")
            } else {
                directory
            };
            paths.insert(mode.clone(), target.clone());
            let manifest = read(&target.join("manifest.json"))?;
            if array(&manifest["entries"], "manifest entries")?
                .iter()
                .any(|entry| {
                    !entry["error"].is_null()
                        || entry["summary"].is_null()
                        || entry["summary"]["issues"]
                            .as_array()
                            .is_some_and(|issues| !issues.is_empty())
                })
            {
                return Err(format!("invalid experiment: {}", target.display()));
            }
            batch.insert(mode.clone(), manifest["entries"].clone());
            observations.push(json!({"round": index, "mode": mode, "harness_seconds": started.elapsed().as_secs_f64(), "capture": capture}));
        }
        for (mode, target) in &paths {
            if mode == "off" {
                continue;
            }
            for entry in array(&plan["entries"], "plan entries")? {
                let name = format!("{}.json", text(entry, "id")?);
                let mut command = Command::new(&binary);
                command
                    .args(["compare"])
                    .arg(paths["off"].join(&name))
                    .arg(target.join(&name))
                    .args(["--rules"])
                    .arg(&rules);
                native::invoke(
                    &mut command,
                    &root.join(format!(
                        "{index}-{mode}-{}-compare.json",
                        text(entry, "id")?
                    )),
                    Duration::from_secs(60),
                )?;
            }
        }
        batches.push(Value::Object(batch));
        let result = json!({
            "version": 1, "unit": "milliseconds", "observations": observations, "timings": aggregate(&batches)?,
            "accounting": "Paired differences of per-batch action medians; no summing phase medians. Metrics retain spec summary semantics: cpu sums evaluation/publication/preparation wall intervals, not thread CPU time. Thread CPU observations live in observation.json. Off retains compiled probes but opens no session. Native includes CPU Counters and signposts. Harness seconds include reporting and native export and are not action timing. This is workload-specific probe perturbation, not an engine speedup.",
        });
        write(&root.join("overhead.json"), &result)?;
    }
    Ok(
        json!({"report": root.join("overhead.json"), "rounds": batches.len(), "modes": config.modes}),
    )
}

fn execute(
    config: &Config,
    source: &Path,
    directory: &Path,
    mode: &str,
    binary: &Path,
) -> Result<Value> {
    if mode == "native" {
        let result = native::record(&native::Config {
            plan: source.into(),
            output: directory.into(),
            binary: binary.into(),
            seconds: config.seconds,
            limit: config.limit,
        })?;
        if result["complete"].as_bool() != Some(true)
            || result["successful"].as_bool() != Some(true)
        {
            return Err(format!("incomplete observation: {}", directory.display()));
        }
        return Ok(result);
    }
    let mut command = Command::new(binary);
    command
        .arg(if mode == "off" { "experiment" } else { "trace" })
        .arg(source)
        .args(["--output"])
        .arg(directory);
    if mode != "off" {
        let mut options = json!({"cpu": mode == "cpu"});
        if let Some(limit) = config.limit {
            options["limit"] = json!(limit);
        }
        command.args(["--options", &options.to_string()]);
    }
    native::invoke(
        &mut command,
        &directory.with_extension("log"),
        Duration::from_secs(config.seconds + 60),
    )?;
    if mode == "off" {
        return Ok(Value::Null);
    }
    let summary = report::write(
        &directory.join("observation.json"),
        &directory.join("analysis"),
    )?;
    if summary["complete"].as_bool() != Some(true) || summary["successful"].as_bool() != Some(true)
    {
        return Err(format!("incomplete observation: {}", directory.display()));
    }
    Ok(
        json!({"complete": summary["complete"], "successful": summary["successful"], "dropped": summary["dropped"], "open": summary["open"]}),
    )
}

fn text<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("{name} must be a string"))
}
