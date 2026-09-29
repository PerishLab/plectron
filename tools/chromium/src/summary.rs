use crate::{Result, read, write};
use serde::Deserialize;
use serde_json::{Map, Number, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub const PHASES: [&str; 6] = [
    "UpdateLayoutTree",
    "Layout",
    "PrePaint",
    "Paint",
    "Layerize",
    "Commit",
];

#[derive(Clone, Deserialize)]
pub struct Event {
    pub pid: i64,
    pub tid: i64,
    pub ts: f64,
    #[serde(default)]
    pub dur: f64,
    pub ph: String,
    pub name: String,
}

pub fn union(intervals: &[(f64, f64)]) -> f64 {
    let mut held = intervals.to_vec();
    held.sort_by(|left, right| left.0.total_cmp(&right.0));
    let mut total = 0.0;
    let mut end = f64::NEG_INFINITY;
    for (start, stop) in held {
        total += (stop - start.max(end)).max(0.0);
        end = end.max(stop);
    }
    total
}

pub fn duration<'a>(events: impl IntoIterator<Item = &'a Event>) -> f64 {
    let mut threads: HashMap<(i64, i64), Vec<(f64, f64)>> = HashMap::new();
    for event in events {
        threads
            .entry((event.pid, event.tid))
            .or_default()
            .push((event.ts, event.ts + event.dur));
    }
    threads
        .values()
        .map(|intervals| union(intervals))
        .sum::<f64>()
        / 1_000.0
}

pub fn topology(events: &[Event], marker: &str) -> Result<Value> {
    let marks: HashMap<_, _> = events
        .iter()
        .filter(|event| event.name.starts_with(&format!("{marker}:")))
        .map(|event| (event.name.as_str(), event))
        .collect();
    let mark = |name: &str| {
        marks
            .get(format!("{marker}:{name}").as_str())
            .copied()
            .ok_or_else(|| format!("missing action marker {marker}:{name}"))
    };
    let begin = mark("start")?;
    let script = mark("script")?;
    let end = mark("end")?;
    if !(begin.ts <= script.ts && script.ts <= end.ts) {
        return Err("unordered action markers".into());
    }
    let clipped: Vec<_> = events
        .iter()
        .filter(|event| event.ph == "X")
        .filter_map(|event| {
            let start = event.ts.max(begin.ts);
            let stop = (event.ts + event.dur).min(end.ts);
            (start < stop).then(|| Event {
                ts: start,
                dur: stop - start,
                ..event.clone()
            })
        })
        .collect();
    let main: Vec<_> = clipped
        .iter()
        .filter(|event| (event.pid, event.tid) == (begin.pid, begin.tid))
        .collect();
    let phases: Map<_, _> = PHASES
        .iter()
        .map(|name| {
            (
                (*name).to_owned(),
                json!(duration(
                    main.iter().copied().filter(|event| event.name == *name)
                )),
            )
        })
        .collect();
    let application = (script.ts - begin.ts) / 1_000.0;
    let phase_events: Vec<_> = main
        .iter()
        .copied()
        .filter(|event| PHASES.contains(&event.name.as_str()))
        .cloned()
        .chain(std::iter::once(Event {
            dur: script.ts - begin.ts,
            ..begin.clone()
        }))
        .collect();
    let named = duration(&phase_events);
    let active = duration(main.iter().copied().filter(|event| event.name == "RunTask"));
    if active + 0.001 < named {
        return Err("named phases are not covered by main-thread RunTask events".into());
    }
    Ok(json!({
        "main": active,
        "application": application,
        "phases": phases,
        "named": named,
        "other": (active - named).max(0.0),
        "window": (end.ts - begin.ts) / 1_000.0,
        "raster": duration(clipped.iter().filter(|event| event.name == "RasterTask")),
        "gpuTasks": duration(clipped.iter().filter(|event| event.name == "GPUTask")),
    }))
}

pub fn spread(values: &[Value]) -> Result<Value> {
    if values.is_empty() {
        return Err("distribution has no samples".into());
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|left, right| number(left).total_cmp(&number(right)));
    let middle = ordered.len() / 2;
    let median = if ordered.len() % 2 == 1 {
        ordered[middle].clone()
    } else {
        numeric((number(&ordered[middle - 1]) + number(&ordered[middle])) / 2.0)?
    };
    let p95 = (ordered.len() * 95).div_ceil(100) - 1;
    Ok(json!({
        "samples": ordered.len(),
        "minimum": ordered[0],
        "median": median,
        "p95": ordered[p95],
        "maximum": ordered[ordered.len() - 1],
    }))
}

pub fn summarize(directory: &Path) -> Result<Value> {
    let mut report = read(&directory.join("report.json"))?;
    let trace = read(&directory.join("trace.json"))?;
    let events: Vec<Event> = serde_json::from_value(trace["traceEvents"].clone())
        .map_err(|error| format!("traceEvents: {error}"))?;
    let mut groups: BTreeMap<(String, String), Vec<Value>> = BTreeMap::new();
    for run in array_mut(&mut report, "runs")? {
        let mode = string(run, "mode")?.to_owned();
        for stage in array_mut(run, "stages")? {
            if mode == "trace" {
                let marker = string(stage, "marker")?;
                stage["trace"] = topology(&events, marker)?;
            }
            let action = string(stage, "action")?.to_owned();
            groups
                .entry((mode.clone(), action))
                .or_default()
                .push(stage.clone());
        }
    }
    let mut actions: Map<String, Value> = Map::new();
    for ((mode, action), stages) in groups {
        let timing = object(&stages[0], "timing")?;
        let metrics: Map<_, _> = timing
            .keys()
            .map(|name| {
                Ok((
                    name.clone(),
                    spread(
                        &stages
                            .iter()
                            .map(|stage| stage["timing"][name].clone())
                            .collect::<Vec<_>>(),
                    )?,
                ))
            })
            .collect::<Result<_>>()?;
        let mut result = json!({
            "metrics": metrics,
            "layouts": spread(&stages.iter().map(|stage| stage["layouts"].clone()).collect::<Vec<_>>())?,
            "styles": spread(&stages.iter().map(|stage| stage["styles"].clone()).collect::<Vec<_>>())?,
        });
        if mode == "trace" {
            let names = [
                "main",
                "application",
                "named",
                "other",
                "window",
                "raster",
                "gpuTasks",
            ];
            result["trace"] = Value::Object(
                names
                    .iter()
                    .map(|name| {
                        Ok((
                            (*name).to_owned(),
                            spread(
                                &stages
                                    .iter()
                                    .map(|stage| stage["trace"][name].clone())
                                    .collect::<Vec<_>>(),
                            )?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            );
            result["phases"] = Value::Object(
                PHASES
                    .iter()
                    .map(|name| {
                        Ok((
                            (*name).to_owned(),
                            spread(
                                &stages
                                    .iter()
                                    .map(|stage| stage["trace"]["phases"][name].clone())
                                    .collect::<Vec<_>>(),
                            )?,
                        ))
                    })
                    .collect::<Result<_>>()?,
            );
            let mut ordered = stages.clone();
            ordered.sort_by(|left, right| {
                number(&left["trace"]["main"]).total_cmp(&number(&right["trace"]["main"]))
            });
            result["representative"] = ordered[ordered.len() / 2]["trace"].clone();
        }
        actions
            .entry(action)
            .or_insert_with(|| Value::Object(Map::new()))[mode] = result;
    }
    let summary = json!({
        "unit": "milliseconds",
        "version": report["version"],
        "actions": actions,
        "accounting": "Trace main = union of RunTask intervals on the marker thread, clipped to the action window. Named phases are inclusive; named unions prevent double counting. Raster and gpuTasks sum per-thread wall intervals, can overlap main, and are not GPU execution time. Independent medians are not additive.",
    });
    write(&directory.join("summary.json"), &summary)?;
    write(&directory.join("stages.json"), &report["runs"])?;
    Ok(summary)
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(f64::NAN)
}

fn numeric(value: f64) -> Result<Value> {
    Number::from_f64(value)
        .map(Value::Number)
        .ok_or_else(|| "distribution produced a non-finite value".into())
}

fn array_mut<'a>(value: &'a mut Value, name: &str) -> Result<&'a mut Vec<Value>> {
    value[name]
        .as_array_mut()
        .ok_or_else(|| format!("{name} must be an array"))
}

fn object<'a>(value: &'a Value, name: &str) -> Result<&'a Map<String, Value>> {
    value[name]
        .as_object()
        .ok_or_else(|| format!("{name} must be an object"))
}

fn string<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("{name} must be a string"))
}
