mod check;
mod stage;

pub use check::validate;

use crate::summary::spread;
use crate::{Result, digest, read};
use check::{array, pixels, text};
use serde_json::{Map, Value, json};
use stage::Stage;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const EXTENT: (u32, u32) = (720, 640);

pub fn difference(before: &Value, after: &Value, path: &str) -> Vec<Value> {
    if before == after {
        return Vec::new();
    }
    if let (Some(left), Some(right)) = (before.as_object(), after.as_object()) {
        let keys: BTreeSet<_> = left.keys().chain(right.keys()).collect();
        return keys
            .into_iter()
            .flat_map(|key| {
                difference(
                    left.get(key).unwrap_or(&Value::Null),
                    right.get(key).unwrap_or(&Value::Null),
                    &format!("{path}/{key}"),
                )
            })
            .collect();
    }
    vec![json!({"path": path, "plectron": before, "chromium": after})]
}

pub fn projection(scenario: &str, value: &Value) -> Value {
    if !matches!(scenario, "article" | "gallery") {
        return value.clone();
    }
    let gallery = scenario == "gallery";
    let boxes = value["boxes"].as_object().cloned().unwrap_or_default();
    let shape = |bounds: &Value| {
        let values = bounds.as_array().cloned().unwrap_or_default();
        Value::Array(
            [0, 2, 3]
                .into_iter()
                .map(|index| rounded(values.get(index).cloned().unwrap_or(Value::Null), gallery))
                .collect(),
        )
    };
    let records: Map<_, _> = boxes
        .iter()
        .filter(|(name, _)| name.starts_with("record-"))
        .map(|(name, bounds)| (name.clone(), shape(bounds)))
        .collect();
    let region = boxes.get("region").cloned().unwrap_or(Value::Null);
    json!({
        "region": region.as_array().map(|items| Value::Array(items.iter().cloned().map(|item| rounded(item, gallery)).collect())).unwrap_or(region),
        "records": records,
        "scroll": value["scroll"],
    })
}

pub fn compare(native: &Path, directory: &Path) -> Result<Value> {
    against(native, directory, pixels)
}

type Groups = BTreeMap<String, Vec<Value>>;

struct Observation<'a, F> {
    engine: &'a str,
    native: &'a Path,
    directory: &'a Path,
    pixels: &'a F,
    issues: Vec<Value>,
}

impl<F> Observation<'_, F>
where
    F: Fn(&Path) -> Result<String>,
{
    fn groups(&mut self, runs: &Value) -> Result<Groups> {
        let mut known: BTreeMap<String, Value> = BTreeMap::new();
        let mut groups = Groups::new();
        for run in array(runs, "runs")? {
            let mut previous: Option<String> = None;
            for stage in array(&run["stages"], "stages")? {
                let action = text(&stage["action"], "action")?.to_owned();
                let observed = self.stage(stage, &action, previous.as_deref())?;
                let held = json!([observed.0.geometry, observed.0.records, observed.1]);
                if known.get(&action).is_some_and(|value| value != &held) {
                    self.issue(&action, "workload or pixels changed between runs");
                }
                known.insert(action.clone(), held);
                previous = Some(observed.1);
                groups.entry(action).or_default().push(observed.0.summary());
            }
        }
        Ok(groups)
    }

    fn stage(
        &mut self,
        stage: &Value,
        action: &str,
        previous: Option<&str>,
    ) -> Result<(Stage, String)> {
        let observed = if self.engine == "plectron" {
            Stage::native(self.native, stage)?
        } else {
            Stage::browser(self.directory, stage)?
        };
        let digest = (self.pixels)(&observed.file)?;
        if self.engine == "chromium" {
            let oracle = self
                .directory
                .join(text(&stage["oracle"], "oracle capture")?);
            if digest != (self.pixels)(&oracle)? {
                self.issue(action, "capture differs from settled oracle");
            }
        }
        let unchanged = previous == Some(digest.as_str());
        match (action, unchanged) {
            ("steady", false) => self.issue(action, "unchanged action changed pixels"),
            ("steady" | "seed", _) | (_, false) => {}
            (_, true) => self.issue(action, "mutation did not change captured pixels"),
        }
        Ok((observed, digest))
    }

    fn issue(&mut self, action: &str, message: &str) {
        self.issues
            .push(json!({"engine": self.engine, "action": action, "issue": message}));
    }
}

fn distributions(stages: &[Value]) -> Result<Value> {
    let names = stages[0]["work"]
        .as_object()
        .ok_or("stage work must be an object")?;
    names
        .keys()
        .map(|name| {
            let samples: Vec<_> = stages
                .iter()
                .map(|stage| stage["work"][name].clone())
                .collect();
            Ok((name.clone(), spread(&samples)?))
        })
        .collect::<Result<Map<_, _>>>()
        .map(Value::Object)
}

fn readiness(stages: &[Value]) -> Result<Value> {
    spread(
        &stages
            .iter()
            .map(|stage| stage["ready"].clone())
            .collect::<Vec<_>>(),
    )
}

fn actions(scenario: &str, ours: &Groups, theirs: &Groups) -> Result<Map<String, Value>> {
    let mut actions = Map::new();
    for (action, values) in ours {
        let other = theirs
            .get(action)
            .ok_or_else(|| format!("browser action is absent: {action}"))?;
        let before = json!({"geometry": projection(scenario, &values[0]["geometry"]), "records": values[0]["records"]});
        let after = json!({"geometry": projection(scenario, &other[0]["geometry"]), "records": other[0]["records"]});
        let differences = difference(&before, &after, "");
        actions.insert(
            action.clone(),
            json!({
                "geometry": differences.is_empty(),
                "differences": differences,
                "variations": difference(&values[0]["geometry"], &other[0]["geometry"], ""),
                "plectron": readiness(values)?,
                "chromium": readiness(other)?,
                "work": {"plectron": distributions(values)?, "chromium": distributions(other)?},
            }),
        );
    }
    Ok(actions)
}

pub fn against<F>(native: &Path, directory: &Path, pixels: F) -> Result<Value>
where
    F: Fn(&Path) -> Result<String>,
{
    let report = read(native)?;
    let exported = directory.join("report.json");
    let browser = read(&exported)?;
    validate(&report, &browser)?;
    let mut issues = Vec::new();
    let mut groups = Vec::new();
    for (engine, runs) in [
        ("plectron", &report["runs"]),
        ("chromium", &browser["runs"]),
    ] {
        let mut observation = Observation {
            engine,
            native,
            directory,
            pixels: &pixels,
            issues,
        };
        groups.push(observation.groups(runs)?);
        issues = observation.issues;
    }
    let scenario = text(&report["case"]["id"], "scenario id")?;
    let actions = actions(scenario, &groups[0], &groups[1])?;
    let mut limits = vec![
        "Native invocation is in-process; Chromium includes CDP command transport and capture scheduling.",
        "PNG compression and renderer antialiasing differ; matching selected geometry is not pixel equivalence.",
        "Chromium Date.now has millisecond resolution; this does not measure physical presentation or pure engine execution.",
    ];
    if matches!(scenario, "article" | "gallery") {
        limits.push("Flow equivalence compares content geometry and scroll state; vertical offsets and extent retain Plectron's non-collapsing margin model as reported variations. Gallery content edges are compared at Plectron's integer-pixel geometry boundary.");
    }
    let matched = actions
        .values()
        .all(|action| action["geometry"].as_bool() == Some(true));
    let evidence = |path: &Path| {
        std::fs::read(path)
            .map(|data| digest(&data))
            .map_err(|error| error.to_string())
    };
    Ok(json!({
        "schema": 1,
        "status": "provisional",
        "passed": issues.is_empty() && matched,
        "analysis_sources": crate::sources(&["compare/mod.rs", "compare/check.rs", "compare/stage.rs", "summary.rs", "paired.rs"])?,
        "classification": "directional capture API observation",
        "unit": "milliseconds",
        "actions": actions,
        "issues": issues,
        "endpoints": {"plectron": report["capture"], "chromium": browser["endpoint"]},
        "evidence": {"plectron": evidence(native)?, "chromium": evidence(&exported)?},
        "identity": {
            "scenario": scenario,
            "count": report["count"],
            "native": select(&report, &["backend", "architecture", "system", "rasterization", "debug"]),
            "config": report["runs"][0]["config"].as_object().map(|values| Value::Object(values.iter().filter(|(key, _)| key.as_str() != "scenario.output").map(|(key, value)| (key.clone(), value.clone())).collect())).unwrap_or_default(),
            "browser": select(&browser, &["version", "host", "sources", "adaptations", "accounting", "prerequisites"]),
            "sampling": select(&browser["config"], &["runs", "warmups"]),
        },
        "ratio": Value::Null,
        "limits": limits,
    }))
}

fn rounded(value: Value, enabled: bool) -> Value {
    if !enabled {
        return value;
    }
    value
        .as_f64()
        .and_then(|number| serde_json::Number::from_f64(number.round_ties_even()))
        .map(Value::Number)
        .unwrap_or(value)
}

fn select(value: &Value, names: &[&str]) -> Value {
    Value::Object(
        names
            .iter()
            .map(|name| ((*name).into(), value[*name].clone()))
            .collect(),
    )
}
