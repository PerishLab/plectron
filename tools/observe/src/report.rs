use crate::{Result, array, integer, read, write_new};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;

pub fn union(intervals: &[(i64, i64)]) -> i64 {
    let mut values = intervals.to_vec();
    values.sort_unstable();
    let mut end = None;
    let mut total = 0;
    for (start, stop) in values {
        let clipped = end.map_or(start, |held| start.max(held));
        total += (stop - clipped).max(0);
        end = Some(end.map_or(stop, |held| held.max(stop)));
    }
    total
}

pub fn analyze(document: &Value) -> Result<(Value, Value)> {
    if document["version"] != 1 || document["unit"] != "nanoseconds" {
        return Err("unsupported observation schema".into());
    }
    let records = array(&document["records"], "records")?;
    let mut by_id = HashMap::new();
    for record in records {
        let identity = integer(&record["id"], "record id")?;
        if by_id.insert(identity, record).is_some() {
            return Err("duplicate span identity".into());
        }
    }
    let mut children: HashMap<i64, Vec<&Value>> = HashMap::new();
    let mut issues = Vec::new();
    for record in records {
        let identity = integer(&record["id"], "record id")?;
        let start = integer(&record["start"], "record start")?;
        let end = integer(&record["end"], "record end")?;
        if end < start {
            return Err("inverted interval".into());
        }
        if record["kind"]["type"] == "receive"
            && integer(&record["kind"]["captured"], "handoff capture")? > start
        {
            return Err("inverted handoff".into());
        }
        let parent = optional_integer(&record["parent"])?;
        if let Some(parent) = parent
            && !by_id.contains_key(&parent)
        {
            issues.push(format!("missing parent {parent}"));
        }
        let enclosing = record
            .get("enclosing")
            .map(optional_integer)
            .transpose()?
            .flatten()
            .or(parent);
        if let Some(enclosing) = enclosing {
            children.entry(enclosing).or_default().push(record);
            if !by_id.contains_key(&enclosing) {
                issues.push(format!("missing enclosing interval {enclosing}"));
            }
        }
        let _ = identity;
    }
    let complete = document["stopped"].as_bool() == Some(true)
        && integer(&document["open"], "open count")? == 0
        && integer(&document["dropped"], "dropped count")? == 0
        && issues.is_empty();
    let roots: Vec<_> = records
        .iter()
        .filter(|record| record["parent"].is_null())
        .collect();
    let successful =
        (!roots.is_empty()).then(|| roots.iter().all(|record| record["outcome"] == "success"));
    let mut groups: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut chrome = Vec::new();
    for record in records {
        let identity = integer(&record["id"], "record id")?;
        let start = integer(&record["start"], "record start")?;
        let end = integer(&record["end"], "record end")?;
        let thread = integer(&record["thread"], "record thread")?;
        let nested = children
            .get(&identity)
            .into_iter()
            .flatten()
            .filter_map(|child| {
                let child_start = child["start"].as_i64()?;
                let child_end = child["end"].as_i64()?;
                (child["thread"].as_i64() == Some(thread) && child_end > start && child_start < end)
                    .then(|| (start.max(child_start), end.min(child_end)))
            })
            .collect::<Vec<_>>();
        let handoff = if record["kind"]["type"] == "receive" {
            Some(start - integer(&record["kind"]["captured"], "handoff capture")?)
        } else {
            None
        };
        let name = record["name"]
            .as_str()
            .ok_or("record name must be a string")?;
        groups.entry(name.into()).or_default().push(json!({
            "wall": end - start,
            "self": complete.then(|| end - start - union(&nested)),
            "cpu": record["cpu"],
            "handoff": handoff,
            "outcome": record["outcome"],
        }));
        if let Some(handoff) = handoff {
            let flow = json!({"name": name, "cat": "handoff", "id": identity.to_string(), "pid": document["process"]});
            let mut started = flow.clone();
            started["ph"] = json!("s");
            started["ts"] = json!((start - handoff) as f64 / 1_000.0);
            started["tid"] = record["kind"]["thread"].clone();
            chrome.push(started);
            let mut finished = flow;
            finished["ph"] = json!("f");
            finished["ts"] = json!(start as f64 / 1_000.0);
            finished["tid"] = json!(thread);
            finished["bp"] = json!("e");
            chrome.push(finished);
        }
        chrome.push(json!({
            "name": name,
            "cat": record["kind"]["type"],
            "ph": "X",
            "ts": start as f64 / 1_000.0,
            "dur": (end - start) as f64 / 1_000.0,
            "pid": document["process"],
            "tid": thread,
            "args": {
                "id": identity,
                "parent": record["parent"],
                "enclosing": record.get("enclosing").cloned().unwrap_or(Value::Null),
                "cpu_ns": record["cpu"],
                "outcome": record["outcome"],
            },
        }));
    }
    let mut summaries = Map::new();
    for (name, values) in groups {
        let metrics: Map<_, _> = ["wall", "self", "cpu", "handoff"]
            .into_iter()
            .map(|key| {
                let samples = values
                    .iter()
                    .map(|value| value[key].as_i64())
                    .collect::<Vec<_>>();
                (key.into(), distribution(&samples))
            })
            .collect();
        let mut outcomes: BTreeMap<String, usize> = BTreeMap::new();
        for value in &values {
            *outcomes
                .entry(value["outcome"].as_str().unwrap_or_default().into())
                .or_default() += 1;
        }
        let mut summary = metrics;
        summary.insert("outcomes".into(), json!(outcomes));
        summaries.insert(name, Value::Object(summary));
    }
    Ok((
        json!({
            "version": 1,
            "unit": "nanoseconds",
            "nesting": if records.iter().all(|record| record.get("enclosing").is_some()) { "explicit" } else { "legacy or mixed: missing enclosing fields fall back to parent" },
            "complete": complete,
            "successful": successful,
            "dropped": document["dropped"],
            "open": document["open"],
            "issues": issues,
            "groups": summaries,
            "accounting": "Wall and CPU inclusive; self wall excludes the union of recorded same-thread execution children, not causal descendants. Nesting is session-local; other sessions and unobserved work cannot be subtracted. Group totals overlap across nesting; do not sum them. Handoff is capture-to-handler latency, including intervening sender work, blocking, queueing and scheduling; it is not CPU or pure queue time. It may overlap enclosing work. Completeness covers recorded intervals, not undelivered envelopes or all work. Instrumentation overhead is included. Dropped counts rejected scope entries; suppressed descendants are uncounted.",
        }),
        json!({"traceEvents": chrome, "displayTimeUnit": "ms"}),
    ))
}

pub fn write(source: &Path, output: &Path) -> Result<Value> {
    let (summary, chrome) = analyze(&read(source)?)?;
    std::fs::create_dir_all(output).map_err(|error| format!("{}: {error}", output.display()))?;
    write_new(&output.join("summary.json"), &summary)?;
    write_new(&output.join("timeline.json"), &chrome)?;
    Ok(summary)
}

fn optional_integer(value: &Value) -> Result<Option<i64>> {
    if value.is_null() {
        Ok(None)
    } else {
        integer(value, "optional identity").map(Some)
    }
}

fn distribution(values: &[Option<i64>]) -> Value {
    let mut values = values.iter().flatten().copied().collect::<Vec<_>>();
    if values.is_empty() {
        return Value::Null;
    }
    values.sort_unstable();
    let count = values.len();
    json!({
        "samples": count,
        "total": values.iter().sum::<i64>(),
        "median": (values[(count - 1) / 2] as f64 + values[count / 2] as f64) / 2.0,
        "maximum": values[count - 1],
    })
}
