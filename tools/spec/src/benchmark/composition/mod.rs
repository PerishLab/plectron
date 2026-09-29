#[cfg(target_os = "macos")]
pub mod caret;
#[cfg(target_os = "macos")]
mod gpu;
mod layout;
pub mod mapping;
mod page;
pub mod scroll;
pub mod visual;
use crate::Result;
use serde::Serialize;
#[derive(Serialize)]
pub struct Benchmark {
    unit: &'static str,
    debug: bool,
    platform: String,
    samples: Vec<Sample>,
}
#[derive(Serialize, Default)]
pub(super) struct Sample {
    scenario: String,
    repetition: usize,
    iterations: usize,
    elapsed: u64,
    planning: u64,
    preparation: u64,
    gpu: u64,
    completed: usize,
    cached: Option<bool>,
    uploads: u64,
    bytes: u64,
    images: Option<webengine::resource::Images>,
    shaping: Option<domengine::Shaping>,
}
pub fn run(platform: (&str, &str)) -> Result<Benchmark> {
    let mut samples = Vec::new();
    for repetition in 0..5 {
        for scenario in ["noop", "swap", "color"] {
            samples.push(page::sample(scenario, repetition)?);
        }
        samples.push(layout::sample(repetition)?);
        #[cfg(target_os = "macos")]
        for scenario in ["repeat", "instances", "visual", "replace"] {
            for index in 0..2 {
                samples.push(gpu::sample(
                    scenario,
                    repetition,
                    (repetition + index).is_multiple_of(2),
                )?);
            }
        }
    }
    Ok(Benchmark {
        unit: "nanoseconds",
        debug: cfg!(debug_assertions),
        platform: format!("{} {}", platform.0, platform.1),
        samples,
    })
}
