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

pub type Benchmark = crate::benchmark::Envelope<crate::benchmark::Native, Sample>;

#[derive(Serialize)]
pub struct Sample {
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
    Ok(Benchmark::native("nanoseconds", platform, samples))
}

pub(super) fn elapsed(start: Instant) -> u64 {
    start.elapsed().as_nanos().min(u64::MAX as u128) as u64
}

#[cfg(target_os = "macos")]
pub mod strips;

#[derive(Serialize)]
pub struct Envelope<H, S> {
    unit: &'static str,
    debug: bool,
    #[serde(flatten)]
    host: H,
    samples: Vec<S>,
}

#[derive(Serialize)]
pub struct Platform {
    platform: String,
}

#[cfg(target_os = "macos")]
#[derive(Serialize)]
pub struct Device {
    device: String,
}

#[derive(Serialize)]
pub struct Native {
    architecture: String,
    system: String,
}

impl<H, S> Envelope<H, S> {
    fn new(unit: &'static str, host: H, samples: Vec<S>) -> Self {
        Self {
            unit,
            debug: cfg!(debug_assertions),
            host,
            samples,
        }
    }
}

impl<S> Envelope<Platform, S> {
    pub fn platform(unit: &'static str, platform: String, samples: Vec<S>) -> Self {
        Self::new(unit, Platform { platform }, samples)
    }
}

#[cfg(target_os = "macos")]
impl<S> Envelope<Device, S> {
    pub fn device(unit: &'static str, device: String, samples: Vec<S>) -> Self {
        Self::new(unit, Device { device }, samples)
    }
}

impl<S> Envelope<Native, S> {
    pub fn native(unit: &'static str, platform: (&str, &str), samples: Vec<S>) -> Self {
        let (architecture, system) = platform;
        let host = Native {
            architecture: architecture.into(),
            system: system.into(),
        };
        Self::new(unit, host, samples)
    }
}
