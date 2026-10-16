mod table;

use crate::{Result, array, integer, read, write};
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use table::{Table, parse};

pub fn intervals(source: &Path, document: &Value) -> Result<(Vec<Value>, Vec<i64>)> {
    let source = std::fs::read_to_string(source).map_err(|error| error.to_string())?;
    let table = Table::new(&source)?;
    let records: HashMap<_, _> = array(&document["records"], "records")?
        .iter()
        .filter_map(|record| record["id"].as_i64().map(|id| (id, record)))
        .collect();
    let mut starts = HashMap::new();
    let mut ends = HashMap::new();
    for row in table.rows() {
        if table.text(row.get("subsystem").copied()) != Some("io.plectron.observe") {
            continue;
        }
        let process = parse(
            table.text(table.child(row.get("process").copied(), "pid")),
            "native process",
        )?;
        if process != integer(&document["process"], "observation process")? {
            continue;
        }
        let identity = parse(
            table.text(row.get("identifier").copied()),
            "signpost identity",
        )?;
        let Some(record) = records.get(&identity) else {
            continue;
        };
        let thread = parse(
            table.text(table.child(row.get("thread").copied(), "tid")),
            "native thread",
        )?;
        if integer(&record["native"], "record native thread")? != thread {
            return Err("native thread identity differs from observation record".into());
        }
        let time = parse(table.text(row.get("time").copied()), "native timestamp")?;
        match table.text(row.get("event-type").copied()) {
            Some("Begin") => {
                let message = table.resolve(row.get("message").copied());
                let numbers = message
                    .into_iter()
                    .flat_map(|node| node.children())
                    .filter(|node| node.has_tag_name("uint64"))
                    .map(|node| parse(table.text(Some(node)), "signpost metadata"))
                    .collect::<Result<Vec<_>>>()?;
                let expected = [
                    integer(&document["session"], "session identity")?,
                    record["parent"].as_i64().unwrap_or(0),
                ];
                (numbers == expected)
                    .then_some(())
                    .ok_or("native session or parent identity mismatch")?;
                starts.insert(identity, (time, thread));
            }
            Some("End") => {
                ends.insert(identity, time);
            }
            _ => {}
        }
    }
    let matched = starts
        .iter()
        .filter_map(|(identity, (start, thread))| {
            let end = *ends.get(identity)?;
            (end >= *start)
                .then(|| json!({"id": identity, "start": start, "end": end, "thread": thread}))
        })
        .collect::<Vec<_>>();
    let mut missing = records
        .values()
        .filter(|record| record["signpost"].as_bool() == Some(true))
        .filter_map(|record| record["id"].as_i64())
        .filter(|identity| {
            !matched
                .iter()
                .any(|window| window["id"].as_i64() == Some(*identity))
        })
        .collect::<Vec<_>>();
    missing.sort_unstable();
    Ok((matched, missing))
}

pub fn samples(source: &Path, process: i64) -> Result<Vec<Value>> {
    let source = std::fs::read_to_string(source).map_err(|error| error.to_string())?;
    let table = Table::new(&source)?;
    let mut result = Vec::new();
    for row in table.rows() {
        if parse(
            table.text(table.child(row.get("process").copied(), "pid")),
            "sample process",
        )? != process
        {
            continue;
        }
        let frames = row
            .get("backtrace")
            .copied()
            .map(|node| table.frames(node))
            .unwrap_or_default();
        result.push(json!({
            "time": parse(table.text(row.get("timestamp").copied()), "sample timestamp")?,
            "thread": parse(table.text(table.child(row.get("thread").copied(), "tid")), "sample thread")?,
            "remark": table.text(row.get("remark").copied()),
            "stack": frames,
        }));
    }
    Ok(result)
}

pub fn assign(windows: &[Value], samples: &[Value]) -> Result<Vec<Value>> {
    let mut threads: HashMap<i64, Vec<&Value>> = HashMap::new();
    for window in windows {
        threads
            .entry(integer(&window["thread"], "window thread")?)
            .or_default()
            .push(window);
    }
    let mut assigned = Vec::new();
    for sample in samples {
        let thread = integer(&sample["thread"], "sample thread")?;
        let time = integer(&sample["time"], "sample time")?;
        let span = threads
            .get(&thread)
            .into_iter()
            .flatten()
            .filter(|window| {
                window["start"].as_i64().is_some_and(|start| start <= time)
                    && window["end"].as_i64().is_some_and(|end| end > time)
            })
            .min_by_key(|window| {
                let start = window["start"].as_i64().unwrap_or_default();
                let end = window["end"].as_i64().unwrap_or_default();
                (
                    end - start,
                    -start,
                    window["id"].as_i64().unwrap_or_default(),
                )
            })
            .and_then(|window| window["id"].as_i64());
        let mut value = sample.clone();
        value["span"] = span.map_or(Value::Null, |identity| json!(identity));
        assigned.push(value);
    }
    Ok(assigned)
}

pub fn correlate(root: &Path) -> Result<Value> {
    let document = read(&root.join("scenario/observation.json"))?;
    let (windows, missing) = intervals(&root.join("os-signpost.xml"), &document)?;
    let assigned = assign(
        &windows,
        &samples(
            &root.join("CountingModeSamples.xml"),
            integer(&document["process"], "process")?,
        )?,
    )?;
    let records: HashMap<_, _> = array(&document["records"], "records")?
        .iter()
        .filter_map(|record| record["id"].as_i64().map(|id| (id, record)))
        .collect();
    let mut groups: BTreeMap<Scope, Vec<&Value>> = BTreeMap::new();
    for sample in &assigned {
        let mut scope = Scope::default();
        let mut identity = sample["span"].as_i64();
        let mut seen = Vec::new();
        while let Some(current) = identity {
            if seen.contains(&current) {
                break;
            }
            seen.push(current);
            let Some(record) = records.get(&current) else {
                break;
            };
            let name = record["name"].as_str().unwrap_or_default();
            scope.visit(name, record);
            identity = record["parent"].as_i64();
        }
        groups.entry(scope).or_default().push(sample);
    }
    let summary = groups
        .into_iter()
        .map(|(scope, rows)| {
            let mut remarks = BTreeMap::new();
            let mut leaves = BTreeMap::new();
            for row in &rows {
                *remarks
                    .entry(row["remark"].as_str().unwrap_or_default().to_owned())
                    .or_insert(0_usize) += 1;
                if let Some(leaf) = row["stack"]
                    .as_array()
                    .and_then(|stack| stack.first())
                    .and_then(Value::as_str)
                {
                    *leaves.entry(leaf.to_owned()).or_insert(0_usize) += 1;
                }
            }
            let mut leaves = leaves.into_iter().collect::<Vec<_>>();
            leaves.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
            leaves.truncate(8);
            json!({"entry": scope.entry, "run": scope.run, "action": scope.action, "node": scope.node, "samples": rows.len(), "remarks": remarks, "leaves": leaves})
        })
        .collect::<Vec<_>>();
    let result = json!({
        "version": 1,
        "matched": windows.len(),
        "missing": missing,
        "samples": assigned.len(),
        "unassigned": assigned.iter().filter(|sample| sample["span"].is_null()).count(),
        "groups": summary,
        "accounting": "Sample counts and native tool classifications, not exact per-node cycles, cache-miss rates or exclusive CPU time. Native collection loss is separate from observation capacity loss.",
    });
    write(&root.join("analysis/native.json"), &result)?;
    write(&root.join("analysis/samples.json"), &Value::Array(assigned))?;
    Ok(result)
}

#[derive(Default, PartialEq, Eq, PartialOrd, Ord)]
struct Scope {
    entry: Option<String>,
    run: Option<String>,
    action: Option<String>,
    node: Option<String>,
}

impl Scope {
    fn visit(&mut self, name: &str, record: &Value) {
        if name.starts_with("entry.") {
            self.entry = Some(name.into());
        }
        if name.starts_with("run.") || name == "warmup" {
            self.run = Some(name.into());
        }
        if name.starts_with("action.") {
            self.action = Some(name.into());
        }
        if self.node.is_none() && record["kind"]["type"] != "graph" {
            self.node = Some(name.into());
        }
    }
}
