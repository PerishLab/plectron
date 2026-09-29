use super::{
    Document, Group, Timing,
    metrics::{self, Observation},
};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct Summary {
    pub case: Option<String>,
    pub runs: usize,
    pub expected: Option<u64>,
    pub unit: &'static str,
    pub issues: Vec<String>,
    pub startup: BTreeMap<String, Option<Timing>>,
    pub actions: BTreeMap<String, Action>,
}
impl Summary {
    pub fn valid(&self) -> bool {
        self.issues.is_empty()
    }
}
#[derive(Serialize)]
pub struct Action {
    pub samples: usize,
    pub frames: usize,
    pub timing: BTreeMap<String, Option<Timing>>,
    pub work: BTreeMap<String, Vec<Observation>>,
    pub output: Vec<Observation>,
    pub maximum: Option<u64>,
}
pub(super) fn summarize(document: &Document) -> Summary {
    let mut stages: BTreeMap<String, Vec<&Value>> = document
        .actions
        .iter()
        .map(|name| (name.clone(), Vec::new()))
        .collect();
    for run in document.runs() {
        for stage in run
            .get("stages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let name = stage.get("action").and_then(Value::as_str).unwrap_or("?");
            stages.entry(name.into()).or_default().push(stage);
        }
    }
    Summary {
        case: document
            .value
            .pointer("/case/id")
            .and_then(Value::as_str)
            .map(Into::into),
        runs: document.runs().len(),
        expected: super::input::repetitions(&document.value),
        unit: "milliseconds",
        issues: document.issues.clone(),
        startup: metrics::STARTUP
            .iter()
            .map(|(name, start, end)| {
                let values = document
                    .runs()
                    .iter()
                    .filter_map(|run| metrics::startup(run, *start, end))
                    .collect();
                ((*name).into(), Timing::collect(values))
            })
            .chain(metrics::LOADING.iter().map(|(name, path)| {
                let values = document
                    .runs()
                    .iter()
                    .filter_map(|run| run.pointer(path)?.as_u64())
                    .collect();
                ((*name).into(), Timing::collect(values))
            }))
            .collect(),
        actions: stages
            .into_iter()
            .map(|(name, stages)| (name, action(&stages)))
            .collect(),
    }
}
fn action(stages: &[&Value]) -> Action {
    let mut work = BTreeMap::new();
    for group in [
        Group::Shaping,
        Group::Painting,
        Group::Geometry,
        Group::Images,
        Group::Routing,
    ] {
        let (base, fields) = group.fields();
        for field in fields {
            let path = format!("{base}/{field}");
            work.insert(
                path.clone(),
                stages.iter().map(|s| metrics::observe(s, &path)).collect(),
            );
        }
    }
    Action {
        samples: stages.len(),
        frames: stages
            .iter()
            .filter(|s| s.get("rendering").is_some_and(Value::is_object))
            .count(),
        timing: metrics::TIMES
            .iter()
            .map(|(name, path)| {
                let values = stages
                    .iter()
                    .filter_map(|s| s.pointer(path)?.as_u64())
                    .collect();
                ((*name).into(), Timing::collect(values))
            })
            .chain(metrics::SUMS.iter().map(|(name, paths)| {
                let values = stages
                    .iter()
                    .filter_map(|stage| metrics::sum(stage, paths))
                    .collect();
                ((*name).into(), Timing::collect(values))
            }))
            .collect(),
        work,
        output: stages
            .iter()
            .map(|s| metrics::observe(s, "/rendering/checksum"))
            .collect(),
        maximum: stages
            .iter()
            .filter_map(|s| s.pointer("/rendering/maximum")?.as_u64())
            .max(),
    }
}
