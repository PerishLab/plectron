use super::report::{Run, Stage};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct Distribution {
    pub samples: usize,
    pub minimum: u64,
    pub median: u64,
    pub p95: u64,
    pub maximum: u64,
}
impl Distribution {
    pub(crate) fn new(mut values: Vec<u64>) -> Option<Self> {
        values.sort_unstable();
        let samples = values.len();
        let minimum = *values.first()?;
        let maximum = *values.last()?;
        let lower = values[(samples - 1) / 2];
        let upper = values[samples / 2];
        Some(Self {
            samples,
            minimum,
            median: lower + (upper - lower) / 2,
            p95: values[samples - samples / 20 - 1],
            maximum,
        })
    }
}
#[derive(Serialize)]
pub struct Reference {
    pub nanoseconds: u64,
    pub exceeded: usize,
}
#[derive(Serialize)]
pub struct Summary {
    pub total: Option<Distribution>,
    pub evaluation: Option<Distribution>,
    pub publication: Option<Distribution>,
    pub inspection: Option<Distribution>,
    pub checkpoint: Option<Distribution>,
    pub collection: Option<Distribution>,
    pub preparation: Option<Distribution>,
    pub submission: Option<Distribution>,
    pub completion: Option<Distribution>,
    pub gpu: Option<Distribution>,
    pub reference: Option<Reference>,
}
impl Summary {
    fn new(stages: &[&Stage], reference: Option<u64>) -> Self {
        Self {
            total: measure(stages, |s| Some(s.total)),
            evaluation: measure(stages, |s| Some(s.evaluation)),
            publication: measure(stages, |s| Some(s.publication)),
            inspection: measure(stages, |s| Some(s.inspection)),
            checkpoint: measure(stages, |s| Some(s.publishing.checkpoint)),
            collection: measure(stages, |s| Some(s.publishing.collection)),
            preparation: measure(stages, |s| s.rendering.as_ref().map(|r| r.preparation)),
            submission: measure(stages, |s| s.rendering.as_ref().map(|r| r.submission)),
            completion: measure(stages, |s| s.rendering.as_ref().map(|r| r.completion)),
            gpu: measure(stages, |s| s.rendering.as_ref().and_then(|r| r.gpu)),
            reference: reference.map(|nanoseconds| Reference {
                nanoseconds,
                exceeded: stages.iter().filter(|s| s.total > nanoseconds).count(),
            }),
        }
    }
}
fn measure(stages: &[&Stage], value: impl Fn(&Stage) -> Option<u64>) -> Option<Distribution> {
    Distribution::new(stages.iter().filter_map(|stage| value(stage)).collect())
}
pub(super) fn collect(runs: &[Run], reference: Option<u64>) -> BTreeMap<String, Summary> {
    let mut groups: BTreeMap<String, Vec<&Stage>> = BTreeMap::new();
    for stage in runs.iter().flat_map(|run| &run.stages) {
        groups.entry(stage.action.clone()).or_default().push(stage);
    }
    groups
        .into_iter()
        .map(|(action, stages)| {
            let reference = reference.filter(|_| action == "edit");
            (action, Summary::new(&stages, reference))
        })
        .collect()
}
