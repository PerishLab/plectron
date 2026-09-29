mod application;
pub mod channel;
pub mod composition;
#[cfg(target_os = "macos")]
pub mod layers;
pub mod layout;
mod numeric;
pub mod paint;
#[cfg(target_os = "macos")]
pub mod vector;
use crate::Result;
use jsengine::jit::{Mode, Report};
use serde::Serialize;
use std::time::Instant;

#[derive(Serialize)]
pub struct Benchmark {
    unit: String,
    architecture: String,
    system: String,
    debug: bool,
    samples: Vec<Sample>,
}

#[derive(Serialize)]
pub(super) struct Sample {
    scenario: String,
    repetition: usize,
    startup: u64,
    cold: u64,
    iterations: usize,
    elapsed: u64,
    report: Report,
}

pub fn run(platform: (&str, &str)) -> Result<Benchmark> {
    let mut samples = Vec::new();
    let modes = [Mode::Off, Mode::Eager, Mode::Hot(32)];
    for repetition in 0..7 {
        for index in 0..3 {
            let mode = modes[(repetition + index) % 3];
            samples.push(numeric::sample(mode, repetition)?);
            samples.push(application::sample(mode, repetition)?);
        }
    }
    Ok(Benchmark {
        unit: "nanoseconds".into(),
        architecture: platform.0.into(),
        system: platform.1.into(),
        debug: cfg!(debug_assertions),
        samples,
    })
}

pub(super) fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

#[cfg(target_os = "macos")]
pub mod strips;
