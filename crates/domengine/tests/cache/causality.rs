use domengine::{Activity, Causality, Input, Layout, Pipeline, Sheet, html};
use fixtures::Text;
use serde::Serialize;
use shared::cache::Manager;
use std::time::Instant;

const COUNTS: [usize; 2] = [64, 512];
const SAMPLES: usize = 30;
const WARMUPS: usize = 3;
const STAGES: [(&str, usize); 3] = [("initial", 512), ("within", 480), ("crossing", 16)];

#[derive(Serialize)]
struct Observation {
    count: usize,
    sample: usize,
    enabled: bool,
    materialization: bool,
    payloads: usize,
    generated: usize,
    action: &'static str,
    total: u128,
    measurement: u64,
    assembly: u64,
    checked: usize,
    retained: usize,
    materialized: usize,
    preparations: usize,
    shapes: usize,
    computed: usize,
    lookups: u64,
    writes: u64,
    resources: usize,
    bytes: usize,
}

fn exercise(
    input: &Input,
    count: usize,
    mode: (usize, bool),
    materialization: bool,
) -> Vec<Observation> {
    let (sample, enabled) = mode;
    let config = config::Store::default();
    config.set("layout.routing", enabled.into());
    config.set("layout.materialization", materialization.into());
    let manager = Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    let mut observations = Vec::new();
    let mut previous = (0, 0);
    for (action, width) in STAGES {
        let start = Instant::now();
        let layout = pipeline.build(input, super::constraints(width)).unwrap();
        let total = start.elapsed().as_nanos();
        let report = pipeline.inspect();
        let stats = manager
            .inspect()
            .into_iter()
            .find(|cache| cache.name == "paragraphs")
            .unwrap()
            .stats;
        let activity = (stats.requests - previous.0, stats.inserted - previous.1);
        previous = (stats.requests, stats.inserted);
        if sample == WARMUPS {
            let reference = Layout::build(input, super::constraints(width), &Text).unwrap();
            assert_eq!(layout.fragments(), reference.fragments());
        }
        if action == "within" {
            assert_eq!(
                report.construction.text.retained,
                if enabled && materialization { count } else { 0 }
            );
            assert_eq!(report.shaping.routed, if enabled { count } else { 0 });
            assert_eq!(report.shaping.materialized, if enabled { 0 } else { count });
        } else {
            assert_eq!(report.shaping.routed, 0);
        }
        observations.push(Observation {
            count,
            sample,
            enabled,
            materialization,
            payloads: report.construction.text.retained,
            generated: report.construction.text.generated,
            action,
            total,
            measurement: report.measurement,
            assembly: report.assembly,
            checked: report.shaping.checked,
            retained: report.shaping.routed,
            materialized: report.shaping.materialized,
            preparations: report.shaping.preparations,
            shapes: report.shaping.shapes,
            computed: report.computed,
            lookups: activity.0,
            writes: activity.1,
            resources: stats.resources,
            bytes: stats.bytes,
        });
    }
    pipeline.close();
    observations
}

#[test]
fn ledger() {
    let document = html::parse("<div><p>A</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let config = config::Store::default();
    config.set("layout.retention", false.into());
    let manager = Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    let mut causality = Causality::new(256);
    pipeline
        .diagnose(&input, super::constraints(128), &mut causality)
        .unwrap();
    pipeline
        .diagnose(&input, super::constraints(128), &mut causality)
        .unwrap();
    assert_eq!(causality.dropped(), 0);
    assert!(
        [
            Activity::Request,
            Activity::Miss,
            Activity::Compute,
            Activity::Screen,
            Activity::Store,
            Activity::Hit,
        ]
        .into_iter()
        .all(|activity| causality
            .events()
            .iter()
            .any(|event| event.activity == activity))
    );
    assert!(
        causality
            .events()
            .iter()
            .any(|event| event.parent.is_some())
    );

    let mut bounded = Causality::new(1);
    pipeline
        .diagnose(&input, super::constraints(128), &mut bounded)
        .unwrap();
    assert_eq!(bounded.events().len(), 1);
    assert!(bounded.dropped() > 0);
}

#[test]
#[ignore = "timing observation, not a correctness check"]
fn attribution() {
    experiment(&[true]);
}

#[test]
#[ignore = "timing observation, not a correctness check"]
fn composition() {
    experiment(&[false, true]);
}

fn experiment(materializations: &[bool]) {
    let mut observations = Vec::new();
    for count in COUNTS {
        let children = (0..count)
            .map(|index| format!("<p>ABCDE {index:04}</p>"))
            .collect::<String>();
        let document = html::parse(&format!(
            "<div style='overflow-wrap: anywhere;'>{children}</div>"
        ))
        .unwrap();
        let input = Input::capture(&document, &Sheet::default()).unwrap();
        for sample in 0..SAMPLES + WARMUPS {
            observations.extend(round(&input, count, sample, materializations));
        }
    }
    println!(
        "{}",
        serde_json::json!({
            "unit": "nanoseconds", "samples": SAMPLES, "warmups": WARMUPS,
            "endpoint": "Pipeline::build with prepared Input and a fresh Pipeline per mode",
            "order": "alternating enabled/disabled within each sample",
            "counts": COUNTS, "stages": STAGES, "observations": observations,
        })
    );
}

fn round(
    input: &Input,
    count: usize,
    sample: usize,
    materializations: &[bool],
) -> Vec<Observation> {
    let mut modes: Vec<_> = materializations
        .iter()
        .flat_map(|value| [(false, *value), (true, *value)])
        .collect();
    if !sample.is_multiple_of(2) {
        modes.reverse();
    }
    modes
        .into_iter()
        .flat_map(|(enabled, materialization)| {
            exercise(input, count, (sample, enabled), materialization)
        })
        .filter(|_| sample >= WARMUPS)
        .collect()
}
