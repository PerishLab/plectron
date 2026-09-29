mod child;
mod process;
mod sample;
use crate::Result;
pub use child::run as child;
use serde::{Deserialize, Serialize};
#[derive(Serialize)]
pub struct Benchmark {
    unit: &'static str,
    debug: bool,
    platform: String,
    samples: Vec<Sample>,
}
#[derive(Serialize)]
pub struct Sample {
    mode: String,
    length: usize,
    iterations: usize,
    repetition: usize,
    setup: u64,
    preparation: u64,
    delivery: u64,
    throughput: f64,
    receiver: Report,
    producer: ipcengine::observe::Snapshot,
}
#[derive(Serialize, Deserialize)]
pub struct Report {
    pub first: Latency,
    pub control: Latency,
    pub bytes: u64,
    pub completed: u64,
    pub waiting: u64,
    pub active: u64,
}
#[derive(Serialize, Deserialize)]
pub struct Latency {
    count: usize,
    p50: Option<u64>,
    p95: Option<u64>,
    maximum: Option<u64>,
}
impl Latency {
    fn new(mut samples: Vec<u64>) -> Self {
        samples.sort_unstable();
        let value = |percent: usize| {
            samples
                .get(samples.len().saturating_sub(1) * percent / 100)
                .copied()
        };
        Self {
            count: samples.len(),
            p50: value(50),
            p95: value(95),
            maximum: samples.last().copied(),
        }
    }
}
pub struct Context {
    pub executable: std::path::PathBuf,
    pub temporary: std::path::PathBuf,
    pub platform: String,
}
pub fn run(context: Context) -> Result<Benchmark> {
    let mut samples = Vec::new();
    for repetition in 0..3 {
        for length in [65_536, 1_048_576, 8_388_608] {
            for index in 0..3 {
                let mode = ["memory", "file", "shared"][(repetition + index) % 3];
                samples.push(sample::run(&context, mode, length, repetition)?);
            }
        }
    }
    Ok(Benchmark {
        unit: "nanoseconds; throughput in MiB/s",
        debug: cfg!(debug_assertions),
        platform: context.platform,
        samples,
    })
}
fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
