use crate::bench::median;
use crate::{Result, array, integer, read, report, write};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

pub fn measured(document: &Value) -> Result<Value> {
    let records = array(&document["records"], "records")?;
    let index: HashMap<_, _> = records
        .iter()
        .filter_map(|record| record["id"].as_i64().map(|id| (id, record)))
        .collect();
    let mut selected = Vec::new();
    for record in records {
        let mut current = record;
        let mut seen = BTreeSet::new();
        while let Some(parent) = current["parent"].as_i64() {
            let identity = integer(&current["id"], "record id")?;
            if !seen.insert(identity) {
                return Err("cyclic observation ancestry".into());
            }
            current = index
                .get(&parent)
                .copied()
                .ok_or_else(|| format!("missing parent {parent}"))?;
        }
        if current["name"] != "warmup" {
            selected.push(record.clone());
        }
    }
    let mut result = document.clone();
    result["records"] = Value::Array(selected);
    Ok(result)
}

pub fn analyze(document: &Value) -> Result<Value> {
    if document["unit"] != "nanoseconds" {
        return Err("unexpected handoff unit".into());
    }
    type Measurements = BTreeMap<String, f64>;
    type Batches = BTreeMap<i64, Measurements>;
    let mut groups: BTreeMap<(String, bool, String), Batches> = BTreeMap::new();
    let mut diagnostics = Vec::new();
    for batch in array(&document["runs"], "handoff runs")? {
        let run = &batch["run"];
        let mode = run["mode"].as_str().map(str::to_owned).unwrap_or_else(|| {
            if run["notified"].as_bool() == Some(true) {
                "both"
            } else {
                "poll"
            }
            .into()
        });
        let key = (
            run["case"].as_str().unwrap_or_default().into(),
            run["traced"].as_bool().unwrap_or(false),
            mode.clone(),
        );
        let round = integer(&batch["round"], "handoff round")?;
        if groups
            .get(&key)
            .is_some_and(|values| values.contains_key(&round))
        {
            return Err("duplicate handoff batch".into());
        }
        let samples = array(&run["samples"], "handoff samples")?;
        let walls = samples
            .iter()
            .map(|sample| number(sample, "wall"))
            .collect::<Result<Vec<_>>>()?;
        let waits = samples
            .iter()
            .map(|sample| number(sample, "waits"))
            .collect::<Result<Vec<_>>>()?;
        let mut result = BTreeMap::from([
            ("wall".into(), median(&walls)?),
            ("waits".into(), median(&waits)?),
            ("p95".into(), percentile(&walls, 95)?),
        ]);
        if !run["report"].is_null() {
            let (summary, _) = report::analyze(&measured(&run["report"])?)?;
            if summary["complete"].as_bool() != Some(true)
                || summary["successful"].as_bool() != Some(true)
            {
                return Err("incomplete handoff capture".into());
            }
            let spans = &summary["groups"];
            result.insert(
                "host_cpu".into(),
                number(&spans["probe.action"]["cpu"]["median"], "host CPU")?,
            );
            result.insert(
                "worker_cpu".into(),
                number(&spans["probe.execute"]["cpu"]["median"], "worker CPU")?,
            );
            result.insert(
                "handoff".into(),
                number(&spans["platform.request"]["handoff"]["median"], "handoff")?,
            );
            diagnostics.push(
                json!({"case": run["case"], "round": round, "mode": mode, "summary": summary}),
            );
        }
        groups.entry(key).or_default().insert(round, result);
    }
    let mut rows = Vec::new();
    for ((case, traced, mode), candidate) in &groups {
        if mode == "poll" {
            continue;
        }
        let batches = groups
            .get(&(case.clone(), *traced, "poll".into()))
            .ok_or("handoff poll baseline is absent")?;
        if candidate.keys().collect::<Vec<_>>() != batches.keys().collect::<Vec<_>>() {
            return Err("unpaired handoff batches".into());
        }
        let metrics = batches
            .values()
            .next()
            .ok_or("handoff batch is empty")?
            .keys();
        for metric in metrics {
            let baseline = batches
                .values()
                .map(|batch| batch[metric])
                .collect::<Vec<_>>();
            let values = candidate
                .values()
                .map(|batch| batch[metric])
                .collect::<Vec<_>>();
            let deltas = values
                .iter()
                .zip(&baseline)
                .map(|(value, base)| value - base)
                .collect::<Vec<_>>();
            let ratios = values
                .iter()
                .zip(&baseline)
                .filter(|(_, base)| **base != 0.0)
                .map(|(value, base)| (value / base - 1.0) * 100.0)
                .collect::<Vec<_>>();
            rows.push(json!({
                "case": case,
                "traced": traced,
                "mode": mode,
                "metric": metric,
                "poll": baseline,
                "notify": values,
                "paired_delta": median(&deltas)?,
                "paired_percent": (!ratios.is_empty()).then(|| median(&ratios)).transpose()?,
            }));
        }
    }
    Ok(json!({
        "unit": "nanoseconds except waits/count and percentages",
        "endpoint": document["endpoint"],
        "build": document["build"],
        "samples": document["samples"],
        "rounds": document["rounds"],
        "limits": document["limits"],
        "comparison": rows,
        "diagnostics": diagnostics,
        "idle": document["idle"],
    }))
}

pub fn summarize(source: &Path, output: &Path) -> Result<Value> {
    let result = analyze(&read(source)?)?;
    crate::Output::directory(output)?;
    write(&output.join("summary.json"), &result)?;
    let omitted = ["diagnostics", "idle"];
    Ok(Value::Object(
        result
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| !omitted.contains(&key.as_str()))
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Map<_, _>>(),
    ))
}

fn number(value: &Value, name: &str) -> Result<f64> {
    value[name]
        .as_f64()
        .ok_or_else(|| format!("{name} must be numeric"))
}

fn percentile(values: &[f64], percent: usize) -> Result<f64> {
    if values.is_empty() {
        return Err("distribution has no samples".into());
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    Ok(values[(values.len() * percent).div_ceil(100) - 1])
}
