pub mod comparison;
pub mod diagnosis;
mod workload;
use crate::Result;
use ::fixtures::Text;
use domengine::{Capture, Input, Layout, Pipeline, Reuse, Sheet, Update, style::Resolver};
use serde::{Deserialize, Serialize};
use shared::cache::{Command, Manager, Snapshot};
use std::hint::black_box;
use std::io::Read;
use std::time::Instant;
use workload::Workload;

#[derive(Serialize)]
pub struct Benchmark {
    schema: u32,
    binary: String,
    unit: String,
    debug: bool,
    platform: String,
    pages: Option<usize>,
    admission: Option<usize>,
    recurrence: Option<usize>,
    window: Option<usize>,
    samples: Vec<Sample>,
}

#[derive(Serialize)]
pub(super) struct Sample {
    scenario: String,
    repetition: usize,
    enabled: bool,
    intrinsic: bool,
    styles: bool,
    incremental: bool,
    update: Update,
    caches: Vec<Snapshot>,
    capture: u64,
    iterations: usize,
    elapsed: u64,
    measurement: u64,
    phases: Phases,
    shape: geoengine::Shape,
    storage: geoengine::Residency,
    traffic: Traffic,
    identity: Identity,
    report: Reuse,
}

#[derive(Default, Serialize)]
struct Identity {
    scanned: usize,
    compared: usize,
    retained: usize,
    fresh: usize,
    children: usize,
    allocated: usize,
}

#[derive(Default, Serialize)]
struct Phases {
    topology: u64,
    planning: u64,
    facts: u64,
    validation: u64,
    identity: u64,
    delta: u64,
}

#[derive(Default, Serialize)]
struct Traffic {
    materialized: usize,
    copied: usize,
}

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Policy {
    pub pages: Option<usize>,
    pub admission: Option<usize>,
    pub recurrence: Option<usize>,
    pub window: Option<usize>,
}

pub fn run(platform: (&str, &str), policy: Policy, binary: &std::path::Path) -> Result<Benchmark> {
    let mut samples = Vec::new();
    for repetition in 0..7 {
        for scenario in ["tiny", "paint", "text", "constraints", "flex", "large"] {
            for index in 0..8 {
                let mode = (repetition + index) % 8;
                samples.push(sample(scenario, repetition, mode, (mode & 1 != 0, policy))?);
            }
        }
        for index in 0..2 {
            samples.push(sample(
                "nested",
                repetition,
                7,
                ((repetition + index) % 2 == 0, policy),
            )?);
        }
    }
    Ok(Benchmark {
        schema: 1,
        binary: fingerprint(binary)?,
        unit: "nanoseconds".into(),
        debug: cfg!(debug_assertions),
        platform: format!("{} {}", platform.0, platform.1),
        pages: policy.pages,
        admission: policy.admission,
        recurrence: policy.recurrence,
        window: policy.window,
        samples,
    })
}

pub(super) fn sample(
    scenario: &str,
    repetition: usize,
    mode: usize,
    settings: (bool, Policy),
) -> Result<Sample> {
    let (intrinsic, policy) = settings;
    let mut workload = Workload::new(scenario)?;
    let enabled = mode & 1 != 0;
    let styles = mode & 2 != 0;
    let incremental = mode & 4 != 0;
    let manager = Manager::default();
    let config = config::Store::default();
    if let Some(pages) = policy.pages {
        config.set("layout.pages", pages.into());
    }
    if let Some(admission) = policy.admission {
        config.set("layout.admission", admission.into());
    }
    if let Some(recurrence) = policy.recurrence {
        config.set("layout.recurrence", recurrence.into());
    }
    if let Some(window) = policy.window {
        config.set("layout.window", window.into());
    }
    let mut pipeline =
        Pipeline::configured(Text, &manager, config).map_err(|error| error.to_string())?;
    let resolver =
        Resolver::managed(Sheet::default(), &manager).map_err(|error| error.to_string())?;
    for cache in manager.inspect() {
        let mut policy = cache.target;
        let active = if cache.name == "intrinsic" {
            intrinsic
        } else if cache.name == "geometry" {
            enabled
        } else {
            styles
        };
        policy.read = active;
        policy.write = active;
        manager
            .command(cache.id, Command::Configure { policy })
            .map_err(|error| error.to_string())?;
    }
    let mut builder = Capture::new(resolver);
    builder.enable(incremental);
    let sheet = Sheet::default();
    for index in 0..4 {
        workload.update(index)?;
        builder
            .update(&workload.document, |input| {
                pipeline.build(input, workload.constraints())
            })
            .map_err(|error| error.to_string())?;
    }
    let start = Instant::now();
    let mut measurement = 0;
    let mut capture = 0;
    let mut last = None;
    let mut phases = Phases::default();
    let mut traffic = Traffic::default();
    let mut identity = Identity::default();
    for index in 0..64 {
        workload.update(index + 4)?;
        let clock = Instant::now();
        let before = measurement;
        let layout = builder
            .update(&workload.document, |input| {
                let clock = Instant::now();
                let result = pipeline.build(input, workload.constraints());
                measurement += super::elapsed(clock);
                result
            })
            .map_err(|error| error.to_string())?;
        capture += super::elapsed(clock).saturating_sub(measurement - before);
        let update = builder.inspect();
        phases.topology += update.timing.topology;
        phases.planning += update.timing.planning;
        phases.facts += update.timing.facts;
        let timing = layout.snapshot().timing();
        phases.validation += timing.validation;
        phases.identity += timing.identity;
        phases.delta += timing.delta;
        let collection = layout.snapshot().collection();
        traffic.materialized += collection.materialized;
        traffic.copied += collection.copied;
        let publication = layout.snapshot().publication();
        identity.scanned += publication.scanned;
        identity.compared += publication.compared;
        identity.retained += publication.retained;
        identity.fresh += publication.fresh;
        identity.children += publication.children;
        identity.allocated += publication.allocated;
        black_box(layout.paint());
        last = Some(layout);
    }
    let elapsed = super::elapsed(start);
    let input = Input::capture(&workload.document, &sheet).map_err(|error| error.to_string())?;
    let expected =
        Layout::build(&input, workload.constraints(), &Text).map_err(|error| error.to_string())?;
    let last = last.unwrap();
    if expected.fragments() != last.fragments() {
        return Err("incremental benchmark differs from full layout".into());
    }
    Ok(Sample {
        scenario: scenario.into(),
        repetition,
        enabled,
        intrinsic,
        styles,
        incremental,
        update: builder.inspect(),
        caches: manager.inspect(),
        capture,
        iterations: 64,
        elapsed,
        measurement,
        phases,
        shape: last.snapshot().shape(),
        storage: last.snapshot().residency(),
        traffic,
        identity,
        report: pipeline.inspect(),
    })
}

pub(super) fn fingerprint(path: &std::path::Path) -> Result<String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hash = 0xcbf29ce484222325u64;
    let mut bytes = [0u8; 16_384];
    loop {
        let read = file.read(&mut bytes).map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        for byte in &bytes[..read] {
            hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
        }
    }
    Ok(format!("fnv64:{hash:016x}"))
}
