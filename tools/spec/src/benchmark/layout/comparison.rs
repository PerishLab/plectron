use super::{Policy, Sample, fingerprint, sample};
use crate::Result;
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Serialize)]
pub struct Comparison {
    schema: u32,
    binary: String,
    unit: String,
    changeunit: String,
    platform: String,
    policies: [Policy; 2],
    summaries: Vec<Summary>,
    observations: Vec<Observation>,
}

#[derive(Serialize)]
struct Observation {
    policy: usize,
    sample: Sample,
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize)]
struct Case {
    scenario: String,
    enabled: bool,
    intrinsic: bool,
    styles: bool,
    incremental: bool,
}

impl Case {
    fn new(scenario: String, flags: [bool; 4]) -> Self {
        Self {
            scenario,
            enabled: flags[0],
            intrinsic: flags[1],
            styles: flags[2],
            incremental: flags[3],
        }
    }
}

struct Trial<'a> {
    scenario: &'a str,
    repetition: usize,
    mode: usize,
    intrinsic: bool,
}

#[derive(Serialize)]
struct Summary {
    case: Case,
    baseline: Metrics,
    candidate: Metrics,
    change: Change,
}

#[derive(Serialize)]
struct Metrics {
    elapsed: u64,
    capture: u64,
    measurement: u64,
    assembly: u64,
    topology: u64,
    planning: u64,
    facts: u64,
    validation: u64,
    identity: u64,
    delta: u64,
    materialized: u64,
    copied: u64,
    cachebytes: u64,
    entries: u64,
    charged: u64,
    nodes: u64,
    blocks: u64,
    flexes: u64,
    texts: u64,
    images: u64,
    minimums: u64,
    windowed: u64,
    windowpeak: u64,
    windowbytes: u64,
    hits: u64,
    computed: u64,
    planned: u64,
}

#[derive(Serialize)]
struct Change {
    elapsed: Option<i64>,
    capture: Option<i64>,
    measurement: Option<i64>,
    cachebytes: Option<i64>,
    copied: Option<i64>,
}

pub fn run(
    platform: (&str, &str),
    policies: [Policy; 2],
    binary: &std::path::Path,
) -> Result<Comparison> {
    let mut observations = Vec::new();
    for repetition in 0..7 {
        for scenario in ["tiny", "paint", "text", "constraints", "flex", "large"] {
            for index in 0..8 {
                let mode = (repetition + index) % 8;
                append(
                    &mut observations,
                    Trial {
                        scenario,
                        repetition,
                        mode,
                        intrinsic: mode & 1 != 0,
                    },
                    policies,
                    repetition + index,
                )?;
            }
        }
        for index in 0..2 {
            append(
                &mut observations,
                Trial {
                    scenario: "nested",
                    repetition,
                    mode: 7,
                    intrinsic: (repetition + index).is_multiple_of(2),
                },
                policies,
                repetition + index,
            )?;
        }
    }
    let summaries = summarize(&observations);
    Ok(Comparison {
        schema: 1,
        binary: fingerprint(binary)?,
        unit: "nanoseconds".into(),
        changeunit: "basispoints".into(),
        platform: format!("{} {}", platform.0, platform.1),
        policies,
        summaries,
        observations,
    })
}

fn append(
    observations: &mut Vec<Observation>,
    trial: Trial<'_>,
    policies: [Policy; 2],
    order: usize,
) -> Result<()> {
    let indices = if order.is_multiple_of(2) {
        [0, 1]
    } else {
        [1, 0]
    };
    for policy in indices {
        observations.push(Observation {
            policy,
            sample: sample(
                trial.scenario,
                trial.repetition,
                trial.mode,
                (trial.intrinsic, policies[policy]),
            )?,
        });
    }
    Ok(())
}

fn summarize(observations: &[Observation]) -> Vec<Summary> {
    let mut groups: BTreeMap<Case, [Vec<&Sample>; 2]> = BTreeMap::new();
    for observation in observations {
        let sample = &observation.sample;
        let case = Case::new(
            sample.scenario.clone(),
            [
                sample.enabled,
                sample.intrinsic,
                sample.styles,
                sample.incremental,
            ],
        );
        groups.entry(case).or_default()[observation.policy].push(sample);
    }
    groups
        .into_iter()
        .map(|(case, samples)| {
            let baseline = metrics(&samples[0]);
            let candidate = metrics(&samples[1]);
            let change = Change {
                elapsed: ratio(baseline.elapsed, candidate.elapsed),
                capture: ratio(baseline.capture, candidate.capture),
                measurement: ratio(baseline.measurement, candidate.measurement),
                cachebytes: ratio(baseline.cachebytes, candidate.cachebytes),
                copied: ratio(baseline.copied, candidate.copied),
            };
            Summary {
                case,
                baseline,
                candidate,
                change,
            }
        })
        .collect()
}

fn metrics(samples: &[&Sample]) -> Metrics {
    Metrics {
        elapsed: median(samples.iter().map(|sample| sample.elapsed)),
        capture: median(samples.iter().map(|sample| sample.capture)),
        measurement: median(samples.iter().map(|sample| sample.measurement)),
        assembly: median(samples.iter().map(|sample| sample.report.assembly)),
        topology: median(samples.iter().map(|sample| sample.phases.topology)),
        planning: median(samples.iter().map(|sample| sample.phases.planning)),
        facts: median(samples.iter().map(|sample| sample.phases.facts)),
        validation: median(samples.iter().map(|sample| sample.phases.validation)),
        identity: median(samples.iter().map(|sample| sample.phases.identity)),
        delta: median(samples.iter().map(|sample| sample.phases.delta)),
        materialized: median(
            samples
                .iter()
                .map(|sample| sample.traffic.materialized as u64),
        ),
        copied: median(samples.iter().map(|sample| sample.traffic.copied as u64)),
        cachebytes: median(samples.iter().map(|sample| sample.report.bytes as u64)),
        entries: median(samples.iter().map(|sample| sample.report.entries as u64)),
        charged: median(samples.iter().map(|sample| sample.report.charged as u64)),
        nodes: median(
            samples
                .iter()
                .map(|sample| sample.report.operations.nodes as u64),
        ),
        blocks: median(
            samples
                .iter()
                .map(|sample| sample.report.operations.blocks as u64),
        ),
        flexes: median(
            samples
                .iter()
                .map(|sample| sample.report.operations.flexes as u64),
        ),
        texts: median(
            samples
                .iter()
                .map(|sample| sample.report.operations.texts as u64),
        ),
        images: median(
            samples
                .iter()
                .map(|sample| sample.report.operations.images as u64),
        ),
        minimums: median(
            samples
                .iter()
                .map(|sample| sample.report.operations.minimums as u64),
        ),
        windowed: median(samples.iter().map(|sample| sample.report.windowed as u64)),
        windowpeak: median(samples.iter().map(|sample| sample.report.windowpeak as u64)),
        windowbytes: median(
            samples
                .iter()
                .map(|sample| sample.report.windowbytes as u64),
        ),
        hits: median(samples.iter().map(|sample| sample.report.hits as u64)),
        computed: median(samples.iter().map(|sample| sample.report.computed as u64)),
        planned: median(samples.iter().map(|sample| sample.report.planned as u64)),
    }
}

fn median(values: impl Iterator<Item = u64>) -> u64 {
    let mut values = values.collect::<Vec<_>>();
    values.sort_unstable();
    values[values.len() / 2]
}

fn ratio(baseline: u64, candidate: u64) -> Option<i64> {
    (baseline != 0).then(|| {
        let change = i128::from(candidate) - i128::from(baseline);
        (change * 10_000 / i128::from(baseline)) as i64
    })
}
