use super::{Document, Group, Timing, input, metrics::Observation, summary::Action};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rules {
    #[serde(default)]
    pub same: Vec<Group>,
    #[serde(default)]
    pub vary: Vec<String>,
}
#[derive(Serialize)]
pub struct Comparison {
    pub compatible: bool,
    pub passed: bool,
    pub unit: &'static str,
    pub rules: Rules,
    pub issues: Vec<String>,
    pub actions: BTreeMap<String, Difference>,
}
#[derive(Serialize)]
pub struct Difference {
    pub samples: [usize; 2],
    pub maximum: [Option<u64>; 2],
    pub output: Change,
    pub work: BTreeMap<String, Change>,
    pub timing: BTreeMap<String, Delta>,
}
#[derive(Serialize)]
pub struct Change {
    pub before: BTreeSet<Observation>,
    pub after: BTreeSet<Observation>,
    pub equal: Option<bool>,
}
impl Change {
    fn new(before: &[Observation], after: &[Observation]) -> Self {
        let before: BTreeSet<_> = before.iter().cloned().collect();
        let after: BTreeSet<_> = after.iter().cloned().collect();
        let available = [&before, &after]
            .iter()
            .all(|values| !values.is_empty() && !values.contains(&Observation::Missing));
        let equal = available.then(|| before.len() == 1 && before == after);
        Self {
            before,
            after,
            equal,
        }
    }
}
#[derive(Serialize)]
pub struct Delta {
    pub before: Option<Timing>,
    pub after: Option<Timing>,
    pub absolute: Option<f64>,
    pub percent: Option<f64>,
}
impl Delta {
    fn new(before: Option<Timing>, after: Option<Timing>) -> Self {
        let absolute = before
            .as_ref()
            .zip(after.as_ref())
            .map(|(a, b)| b.median - a.median);
        let percent = before
            .as_ref()
            .filter(|a| a.median != 0.0)
            .zip(absolute)
            .map(|(a, delta)| delta / a.median * 100.0);
        Self {
            before,
            after,
            absolute,
            percent,
        }
    }
}
pub fn compare(before: &Document, after: &Document, rules: &Rules) -> Comparison {
    let mut issues = Vec::new();
    for (label, report) in [("before", before), ("after", after)] {
        issues.extend(
            report
                .issues
                .iter()
                .map(|issue| format!("{label}: {issue}")),
        );
    }
    for path in input::IDENTITY {
        if before.value.pointer(path) != after.value.pointer(path) {
            issues.push(format!("incompatible identity {path}"));
        }
    }
    metadata(before, after, rules, &mut issues);
    let mut compatible = issues.is_empty();
    let a = before.summary();
    let b = after.summary();
    let mut actions = BTreeMap::new();
    for (name, previous) in &a.actions {
        let Some(next) = b.actions.get(name) else {
            continue;
        };
        let difference = difference(previous, next);
        assess(&difference.output, name, &mut compatible, &mut issues);
        for group in &rules.same {
            let (base, fields) = group.fields();
            for field in fields {
                let path = format!("{base}/{field}");
                assess(
                    &difference.work[&path],
                    &format!("{name}{path}"),
                    &mut compatible,
                    &mut issues,
                );
            }
        }
        actions.insert(name.clone(), difference);
    }
    Comparison {
        compatible,
        passed: issues.is_empty(),
        unit: "milliseconds",
        rules: rules.clone(),
        issues,
        actions,
    }
}
fn assess(change: &Change, name: &str, compatible: &mut bool, issues: &mut Vec<String>) {
    match change.equal {
        Some(true) => (),
        Some(false) => issues.push(format!("{name}: changed or nondeterministic values")),
        None => {
            *compatible = false;
            issues.push(format!("{name}: required observation unavailable"));
        }
    }
}
fn difference(a: &Action, b: &Action) -> Difference {
    Difference {
        samples: [a.samples, b.samples],
        maximum: [a.maximum, b.maximum],
        output: Change::new(&a.output, &b.output),
        work: a
            .work
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    Change::new(value, b.work.get(key).map_or(&[], Vec::as_slice)),
                )
            })
            .collect(),
        timing: a
            .timing
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    Delta::new(value.clone(), b.timing.get(key).cloned().flatten()),
                )
            })
            .collect(),
    }
}
fn metadata(before: &Document, after: &Document, rules: &Rules, issues: &mut Vec<String>) {
    let Some(reference) = before.runs().first() else {
        return;
    };
    let baseline = signature(reference, rules);
    for (label, document) in [("before", before), ("after", after)] {
        for (index, run) in document.runs().iter().enumerate() {
            mismatch(
                &baseline,
                &signature(run, rules),
                &format!("{label} run {index}"),
                issues,
            );
        }
    }
}
fn mismatch(before: &Value, after: &Value, location: &str, issues: &mut Vec<String>) {
    for group in ["config", "assets"] {
        let keys: BTreeSet<_> = [&before[group], &after[group]]
            .into_iter()
            .filter_map(Value::as_object)
            .flat_map(|object| object.keys())
            .collect();
        for key in keys {
            let a = before[group].get(key);
            let b = after[group].get(key);
            if a != b {
                issues.push(format!(
                    "{location}: {group}.{key} changed: {} -> {}",
                    a.map_or("missing".into(), Value::to_string),
                    b.map_or("missing".into(), Value::to_string)
                ));
            }
        }
    }
}
fn signature(run: &Value, rules: &Rules) -> Value {
    let mut config = run["config"].clone();
    if let Some(config) = config.as_object_mut() {
        for key in rules
            .vary
            .iter()
            .map(String::as_str)
            .chain(["scenario.runs", "scenario.reference"])
        {
            config.remove(key);
        }
    }
    let assets: BTreeMap<_, _> = run["assets"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(url, asset)| {
            (
                url,
                serde_json::json!([asset.get("bytes"), asset.get("fingerprint")]),
            )
        })
        .collect();
    serde_json::json!({"config": config, "assets": assets})
}
