use super::{Case, ITERATIONS, WARMUP};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use webengine::{Fault, Snapshot};

#[derive(Serialize)]
pub struct Benchmark {
    version: u8,
    unit: &'static str,
    architecture: &'static str,
    system: &'static str,
    debug: bool,
    workloads: Vec<Workload>,
    samples: Vec<Sample>,
    accounting: &'static str,
}

#[derive(Serialize)]
struct Workload {
    id: &'static str,
    width: usize,
    height: usize,
    scale: f64,
    warmup: usize,
    iterations: usize,
    property: &'static str,
    pacing: &'static str,
}

#[derive(Serialize)]
pub(super) struct Sample {
    workload: &'static str,
    backend: &'static str,
    repetition: usize,
    device: Option<String>,
    stages: BTreeMap<&'static str, Distribution>,
    engine: Engine,
    signature: Signature,
}

#[derive(Serialize)]
struct Distribution {
    samples: usize,
    total: u64,
    minimum: u64,
    median: u64,
    p95: u64,
    maximum: u64,
}

#[derive(Serialize)]
struct Engine {
    sampling: u64,
    compositing: u64,
    animations: u64,
    effects: u64,
    layouts: u64,
    paints: u64,
    stale: u64,
}

#[derive(Serialize)]
struct Signature {
    frames: usize,
    distinct: usize,
    maximum: u32,
    checksum: u64,
}

#[derive(Default)]
pub(super) struct Timings {
    pub pulse: Vec<u64>,
    pub frame: Vec<u64>,
    pub preparation: Vec<u64>,
    pub submission: Vec<u64>,
    pub completion: Vec<u64>,
    pub gpu: Vec<u64>,
    pub acknowledge: Vec<u64>,
}

pub(super) struct Capture {
    pub case: Case,
    pub backend: &'static str,
    pub repetition: usize,
    pub device: Option<String>,
    pub timings: Timings,
    pub before: Snapshot,
    pub after: Snapshot,
    pub checksums: Vec<u64>,
    pub maximum: u32,
}

impl Benchmark {
    pub(super) fn new(
        platform: (&'static str, &'static str),
        cases: &[Case],
        samples: Vec<Sample>,
    ) -> Result<Self, Fault> {
        validate(&samples, cases)?;
        Ok(Self {
            version: 1,
            unit: "nanoseconds",
            architecture: platform.0,
            system: platform.1,
            debug: cfg!(debug_assertions),
            workloads: cases.iter().copied().map(Workload::from).collect(),
            samples,
            accounting: "Stage distributions are per frame and overlap engine counters; do not sum them. Pulse includes animation sampling, effect publication and scene compositing. Completion is host-observed backend completion; GPU is command-buffer execution when available. Readback, CPU oracle and checksum work are outside timed stages. The experiment is warm, windowless and excludes hardware scanout.",
        })
    }
}

impl From<Case> for Workload {
    fn from(case: Case) -> Self {
        Self {
            id: case.id,
            width: case.width,
            height: case.height,
            scale: case.scale,
            warmup: WARMUP,
            iterations: ITERATIONS,
            property: "opacity",
            pacing: "animation deadline",
        }
    }
}

impl Capture {
    pub fn sample(self) -> Result<Sample, Fault> {
        let computation = self.after.computation.since(self.before.computation);
        let compositing = self.after.compositing;
        let engine = Engine {
            sampling: computation.sampling,
            compositing: compositing.elapsed - self.before.compositing.elapsed,
            animations: computation.animations,
            effects: computation.effects,
            layouts: computation.layouts,
            paints: computation.paints,
            stale: compositing.stale - self.before.compositing.stale,
        };
        let distinct = self.checksums.iter().copied().collect::<BTreeSet<_>>();
        let signature = Signature {
            frames: self.checksums.len(),
            distinct: distinct.len(),
            maximum: self.maximum,
            checksum: self
                .checksums
                .iter()
                .fold(0, |total, value| total.wrapping_mul(1099511628211) ^ value),
        };
        Ok(Sample {
            workload: self.case.id,
            backend: self.backend,
            repetition: self.repetition,
            device: self.device,
            stages: stages(self.timings)?,
            engine,
            signature,
        })
    }
}

fn stages(timings: Timings) -> Result<BTreeMap<&'static str, Distribution>, Fault> {
    let mut stages = BTreeMap::new();
    stages.insert("pulse", distribution(timings.pulse)?);
    stages.insert("frame", distribution(timings.frame)?);
    stages.insert("preparation", distribution(timings.preparation)?);
    stages.insert("submission", distribution(timings.submission)?);
    stages.insert("completion", distribution(timings.completion)?);
    if !timings.gpu.is_empty() {
        stages.insert("gpu", distribution(timings.gpu)?);
    }
    stages.insert("acknowledge", distribution(timings.acknowledge)?);
    Ok(stages)
}

fn distribution(mut values: Vec<u64>) -> Result<Distribution, Fault> {
    if values.len() != ITERATIONS {
        return Err(Fault("caret timing sample count mismatch".into()));
    }
    values.sort_unstable();
    let middle = values.len() / 2;
    let median = values[middle - 1] + (values[middle] - values[middle - 1]) / 2;
    let p95 = values[(values.len() * 95).div_ceil(100) - 1];
    Ok(Distribution {
        samples: values.len(),
        total: values.iter().sum(),
        minimum: values[0],
        median,
        p95,
        maximum: *values.last().unwrap(),
    })
}

fn validate(samples: &[Sample], cases: &[Case]) -> Result<(), Fault> {
    for sample in samples {
        semantic(sample)?;
    }
    for case in cases {
        for repetition in 0..3 {
            equivalent(samples, *case, repetition)?;
        }
    }
    Ok(())
}

fn semantic(sample: &Sample) -> Result<(), Fault> {
    if sample.engine.animations != ITERATIONS as u64 {
        return Err(Fault("caret animation count mismatch".into()));
    }
    if sample.engine.effects != ITERATIONS as u64 {
        return Err(Fault("caret effect count mismatch".into()));
    }
    if sample.engine.layouts != 0 || sample.engine.paints != 0 {
        return Err(Fault("caret effect rebuilt content".into()));
    }
    if sample.engine.stale != 0 {
        return Err(Fault("caret effect became stale".into()));
    }
    if sample.signature.frames != ITERATIONS || sample.signature.distinct != 2 {
        return Err(Fault("caret output signature mismatch".into()));
    }
    if sample.signature.maximum > 2 {
        return Err(Fault("caret backend output differs".into()));
    }
    Ok(())
}

fn equivalent(samples: &[Sample], case: Case, repetition: usize) -> Result<(), Fault> {
    let pair: Vec<_> = samples
        .iter()
        .filter(|sample| sample.workload == case.id && sample.repetition == repetition)
        .collect();
    if pair.len() != 2 || pair[0].signature.checksum != pair[1].signature.checksum {
        return Err(Fault("caret backends produced different output".into()));
    }
    Ok(())
}

pub(super) fn difference(expected: &[u32], actual: &[u32]) -> Result<u32, Fault> {
    if expected.len() != actual.len() {
        return Err(Fault("caret backend output length mismatch".into()));
    }
    Ok(expected
        .iter()
        .zip(actual)
        .flat_map(|(expected, actual)| {
            [0, 8, 16].map(|shift| ((expected >> shift) & 255).abs_diff((actual >> shift) & 255))
        })
        .max()
        .unwrap_or(0))
}

pub(super) fn checksum(pixels: &[u32]) -> u64 {
    pixels.iter().fold(14695981039346656037, |value, pixel| {
        value.wrapping_mul(1099511628211) ^ u64::from(*pixel)
    })
}
