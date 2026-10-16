mod workload;
use crate::Result;
use domengine::{Painter, Painting};
use graphics::shared::{Backend, Cpu, Frame, Scene, Visual};
use serde::Serialize;
use shared::cache::{Manager, Snapshot};
use std::hint::black_box;
use std::time::Instant;

pub type Benchmark = crate::benchmark::Envelope<crate::benchmark::Platform, Sample>;

#[derive(Serialize)]
pub struct Sample {
    scenario: String,
    repetition: usize,
    mode: String,
    iterations: usize,
    recording: u64,
    composition: u64,
    planning: u64,
    report: Option<Painting>,
    caches: Vec<Snapshot>,
}

pub fn run(platform: (&str, &str)) -> Result<Benchmark> {
    let mut samples = Vec::new();
    let modes = ["reference", "fresh", "retained"];
    for repetition in 0..7 {
        for scenario in ["tiny", "text", "placement", "color", "transform"] {
            let layouts = workload::build(scenario)?;
            for index in 0..3 {
                let mode = modes[(repetition + index) % 3];
                samples.push(sample(&layouts, scenario, repetition, mode)?);
            }
        }
    }
    Ok(Benchmark::platform(
        "nanoseconds",
        format!("{} {}", platform.0, platform.1),
        samples,
    ))
}

fn sample(
    layouts: &[domengine::Layout],
    scenario: &str,
    repetition: usize,
    mode: &str,
) -> Result<Sample> {
    let manager = Manager::default();
    let mut painter = Painter::managed(&manager).map_err(|error| error.to_string())?;
    painter.enable(mode == "retained");
    let mut sample = Sample {
        scenario: scenario.into(),
        repetition,
        mode: mode.into(),
        iterations: 64,
        recording: 0,
        composition: 0,
        planning: 0,
        report: None,
        caches: Vec::new(),
    };
    let mut last = None;
    for (index, layout) in layouts.iter().enumerate() {
        let clock = Instant::now();
        let items = if mode == "reference" {
            layout.paint()
        } else {
            painter.build(layout).map_err(|error| error.to_string())?
        };
        let recording = super::elapsed(clock);
        let clock = Instant::now();
        let scene = Scene::new(layout.extent(), layout.revision(), items)
            .map_err(|error| error.to_string())?;
        let frame =
            Frame::new(scene, Visual::new(layout.extent())).map_err(|error| error.to_string())?;
        let composition = super::elapsed(clock);
        let clock = Instant::now();
        black_box(frame.plan().map_err(|error| error.to_string())?);
        let planning = super::elapsed(clock);
        if index >= 4 {
            sample.recording += recording;
            sample.composition += composition;
            sample.planning += planning;
        }
        last = Some(frame);
    }
    let layout = layouts.last().unwrap();
    let scene = Scene::new(layout.extent(), layout.revision(), layout.paint())
        .map_err(|error| error.to_string())?;
    let expected =
        Frame::new(scene, Visual::new(layout.extent())).map_err(|error| error.to_string())?;
    if Cpu.render(&expected).map_err(|error| error.to_string())?
        != Cpu
            .render(&last.unwrap())
            .map_err(|error| error.to_string())?
    {
        return Err("recording benchmark differs from reference pixels".into());
    }
    if mode != "reference" {
        sample.report = Some(painter.inspect());
        sample.caches = manager.inspect();
    }
    Ok(sample)
}
