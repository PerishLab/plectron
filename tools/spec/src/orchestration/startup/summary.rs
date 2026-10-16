use super::{MILESTONES, gather, summarize};
use crate::Result;
use serde_json::{Map, Value, json};

pub(super) fn lifecycle(record: &Value) -> Result<Value> {
    let admitted = integer(record, "admitted")?;
    let produced = integer(record, "produced")?;
    let detected = integer(record, "detected")?;
    if admitted > produced || produced > detected {
        return Err("first-frame timestamps are not monotonic".into());
    }
    let visible = record.get("visible").and_then(Value::as_u64);
    let seen = record.get("seen").and_then(Value::as_u64);
    if let Some(visible) = visible {
        let seen = seen.ok_or("visible first frame has no seen timestamp")?;
        if produced > visible || visible > seen {
            return Err("first-frame visibility timestamps are not monotonic".into());
        }
    }
    Ok(json!({
        "submitted": admitted, "backend_completed": produced,
        "visible": visible, "seen": seen, "drops": record["drops"],
        "interactive": detected, "submit_to_backend": produced - admitted,
        "backend_to_interactive": detected - produced,
        "backend_to_visible": visible.map(|value| value - produced),
        "visible_to_detection": visible.zip(seen).map(|(visible, seen)| seen - visible),
    }))
}

fn integer(value: &Value, key: &str) -> Result<u64> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("first frame has no {key} timestamp"))
}

fn deltas(selected: &[&Value], pair: &[&str]) -> Result<Value> {
    let milestone = |sample: &Value, name: &str| {
        sample["milestones"][name]
            .as_u64()
            .ok_or_else(|| format!("first frame is missing the {name} milestone"))
    };
    selected
        .iter()
        .map(|sample| {
            Ok(json!(
                milestone(sample, pair[1])? - milestone(sample, pair[0])?
            ))
        })
        .collect::<Result<Vec<_>>>()
        .map(Value::Array)
}

fn backend(selected: &[&Value]) -> Result<Value> {
    let mut columns = Map::new();
    for key in [
        "submitted",
        "backend_completed",
        "interactive",
        "submit_to_backend",
        "backend_to_interactive",
        "drops",
    ] {
        columns.insert(key.into(), gather(selected, &["lifecycle", key])?);
    }
    let visible = selected
        .iter()
        .copied()
        .filter(|sample| !sample["lifecycle"]["visible"].is_null())
        .collect::<Vec<_>>();
    let shown = [
        "visible",
        "seen",
        "backend_to_visible",
        "visible_to_detection",
    ];
    for key in shown.into_iter().filter(|_| !visible.is_empty()) {
        columns.insert(key.into(), gather(&visible, &["lifecycle", key])?);
    }
    columns.insert("observed".into(), gather(selected, &["observed"])?);
    for name in MILESTONES {
        columns.insert(name.into(), gather(selected, &["milestones", name])?);
    }
    for pair in MILESTONES.windows(2) {
        columns.insert(
            format!("{}_to_{}", pair[0], pair[1]),
            deltas(selected, pair)?,
        );
    }
    let mut summary = summarize(columns)?;
    summary.as_object_mut().unwrap().insert(
        "visibility_coverage".into(),
        json!({"observed": visible.len(), "total": selected.len()}),
    );
    Ok(summary)
}

pub(super) fn coverage(samples: &[Value], backends: &[&str], runs: usize) -> Result<Value> {
    let mut summaries = Map::new();
    for name in backends {
        let selected = samples
            .iter()
            .filter(|sample| sample["backend"] == *name)
            .collect::<Vec<_>>();
        summaries.insert((*name).into(), backend(&selected)?);
    }
    Ok(json!({
        "unit": "nanoseconds", "samples_per_backend": runs, "summary": summaries,
        "cache": "fresh application process; OS file and driver caches uncontrolled",
        "clock": "submitted, backend_completed, visible and interactive share the platform lifetime monotonic clock",
        "milestones": "loaded through presentation share the page-relative monotonic clock",
        "endpoint": {
            "submitted": "first frame admitted immediately before backend rendering",
            "backend_completed": "backend completion timestamp; Metal includes its presentation copy",
            "visible": "physical drawable presentation when the platform supplies evidence; absent otherwise",
            "interactive": "backend completion consumed and published to the page; logical input can be accepted independently of visibility",
            "observed": "parent-process detection through passive IPC; polling makes this an upper bound",
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic() {
        let record = json!({"admitted": 3, "produced": 2, "detected": 4, "drops": 0});
        assert!(lifecycle(&record).is_err());
    }

    #[test]
    fn stages() {
        let sample = json!({
            "backend": "metal", "observed": 20,
            "lifecycle": {"submitted": 1, "backend_completed": 3, "interactive": 5,
                "submit_to_backend": 2, "backend_to_interactive": 2, "drops": 0,
                "visible": 4, "seen": 5, "backend_to_visible": 1,
                "visible_to_detection": 1},
            "milestones": {"loaded": 1, "runtime": 2, "activation": 4, "scene": 7,
                "publication": 11, "frame": 16, "presentation": 22}
        });
        let summary = coverage(&[sample], &["metal"], 1).unwrap();
        assert_eq!(
            summary["summary"]["metal"]["loaded_to_runtime"]["median"],
            1
        );
        assert_eq!(
            summary["summary"]["metal"]["visibility_coverage"]["observed"],
            1
        );
    }
}
