pub mod motion;
mod workload;
use graphics::metal::{Metal, Resource};
use graphics::shared::{
    Backend, Cpu, Evaluation, Output, Pass, Preparation, Primitive, Rasterization, State,
};
use serde::Serialize;
use shared::cache::Command;
use std::time::{Duration, Instant};

pub type Benchmark = crate::benchmark::Envelope<crate::benchmark::Device, Sample>;
#[derive(Default, Serialize)]
pub struct Sample {
    scenario: String,
    mode: String,
    repetition: usize,
    iterations: usize,
    preparation: u64,
    generation: u64,
    builds: u64,
    visits: u64,
    crossings: u64,
    encoding: u64,
    gpu: u64,
    completed: usize,
    uploads: u64,
    uploaded: u64,
    retained: usize,
    residency: usize,
    strips: usize,
    boundary: usize,
    solid: usize,
    maximum: u32,
    checksum: u64,
}
pub fn run() -> crate::Result<Benchmark> {
    measure().map_err(|fault| fault.to_string())
}
fn measure() -> Result<Benchmark, graphics::shared::Fault> {
    let mut metal = Metal::new()?;
    let mut samples = Vec::new();
    for repetition in 0..3 {
        for scenario in ["dense", "large", "repeat", "scale"] {
            for index in 0..3 {
                let mode = ["direct", "fresh", "resident"][(index + repetition) % 3];
                let sample = sample(&mut metal, scenario, mode, repetition)?;
                record(&mut samples, sample)?;
            }
        }
    }
    Ok(Benchmark::device("nanoseconds", metal.name(), samples))
}
fn sample(
    metal: &mut Metal,
    scenario: &str,
    mode: &str,
    repetition: usize,
) -> Result<Sample, graphics::shared::Fault> {
    let frame = super::vector::workload::frame(scenario)?;
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    if mode != "direct" {
        preparation.rasterization(Rasterization::Strips);
    }
    configure(metal, &mut preparation, mode)?;
    let resource = if mode == "direct" {
        Resource::Paths
    } else {
        Resource::Coverages
    };
    let mut report = Sample {
        scenario: scenario.into(),
        mode: mode.into(),
        repetition,
        iterations: 8,
        ..Sample::default()
    };
    let mut before = preparation.rasterizer().report();
    let mut residency = metal.inspect(resource);
    for iteration in 0..10 {
        let scale = if scenario == "scale" && iteration % 2 == 0 {
            2.0
        } else {
            1.0
        };
        let start = Instant::now();
        let pass = preparation.prepare(&frame, Output::new(frame.extent(), scale)?)?;
        let planning = super::elapsed(start);
        let start = Instant::now();
        let mut work = metal.submit(&pass)?;
        let encoding = super::elapsed(start);
        wait(metal, &mut work)?;
        if iteration < 2 {
            before = preparation.rasterizer().report();
            residency = metal.inspect(resource);
            continue;
        }
        report.preparation += planning;
        report.encoding += encoding;
        if let Some(duration) = work.duration() {
            report.gpu += duration.as_nanos() as u64;
            report.completed += 1;
        }
        count(pass.layer(), &mut report);
        if iteration >= 8 {
            verify((metal, &work), (&frame, &pass), &mut report)?;
        }
    }
    let after = preparation.rasterizer().report();
    report.generation = after.generation - before.generation;
    report.builds = after.builds - before.builds;
    report.visits = after.visits - before.visits;
    report.crossings = after.crossings - before.crossings;
    report.uploads = metal.inspect(resource).uploads - residency.uploads;
    report.uploaded = metal.inspect(resource).bytes - residency.bytes;
    report.retained = preparation.rasterizer().inspect().stats.bytes;
    report.residency = metal.resources(resource).stats.bytes;
    Ok(report)
}
fn configure(
    metal: &mut Metal,
    preparation: &mut Preparation,
    mode: &str,
) -> Result<(), graphics::shared::Fault> {
    for resource in [Resource::Paths, Resource::Coverages] {
        metal.cache(resource, Command::Clear)?;
        let mut policy = metal.resources(resource).active;
        policy.read = mode != "fresh";
        policy.write = policy.read;
        metal.cache(resource, Command::Configure { policy })?;
    }
    let mut policy = preparation.rasterizer().inspect().active;
    policy.read = mode != "fresh";
    policy.write = policy.read;
    preparation
        .rasterizer()
        .command(Command::Configure { policy })
}
fn count(layer: &graphics::shared::Layer, report: &mut Sample) {
    for primitive in &layer.primitives {
        match primitive {
            Primitive::Coverage(placement, _) => {
                let coverage = &placement.coverage;
                report.strips += coverage.strips().len();
                report.boundary += coverage.samples().len();
                report.solid += coverage
                    .strips()
                    .iter()
                    .filter(|strip| matches!(strip.mask, graphics::shared::coverage::Mask::Solid))
                    .map(|strip| strip.width)
                    .sum::<usize>();
            }
            Primitive::Group(group) => count(group, report),
            _ => (),
        }
    }
}
fn verify(
    hardware: (&mut Metal, &graphics::metal::Work),
    data: (&graphics::shared::Frame, &Pass),
    report: &mut Sample,
) -> Result<(), graphics::shared::Fault> {
    let actual = hardware.0.read(hardware.1)?;
    let expected = Cpu.submit(data.1)?;
    let mut oracle = Preparation::default();
    oracle.evaluation(Evaluation::Quadratic);
    let direct = Cpu.submit(&oracle.prepare(data.0, data.1.output())?)?;
    if expected != direct
        || actual.extent != direct.extent
        || actual.pixels.len() != direct.pixels.len()
    {
        return Err(graphics::shared::Fault(
            "strip CPU output differs from direct evaluation".into(),
        ));
    }
    for (a, b) in actual.pixels.iter().zip(&direct.pixels) {
        let difference = [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255));
        report.maximum = report.maximum.max(*difference.iter().max().unwrap());
        report.checksum = report.checksum.wrapping_mul(1099511628211) ^ u64::from(*b);
    }
    if report.maximum > 1 {
        return Err(graphics::shared::Fault(
            "strip Metal output differs from CPU".into(),
        ));
    }
    Ok(())
}
fn record(samples: &mut Vec<Sample>, sample: Sample) -> Result<(), graphics::shared::Fault> {
    if samples.iter().any(|previous| {
        previous.scenario == sample.scenario && previous.checksum != sample.checksum
    }) {
        return Err(graphics::shared::Fault(
            "strip benchmark checksum mismatch".into(),
        ));
    }
    samples.push(sample);
    Ok(())
}
fn wait(
    metal: &mut Metal,
    work: &mut graphics::metal::Work,
) -> Result<(), graphics::shared::Fault> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match metal.poll(work)? {
            State::Complete => return Ok(()),
            State::Failed(fault) => return Err(fault),
            State::Pending => (),
        }
        if Instant::now() > deadline {
            return Err(graphics::shared::Fault("strip GPU deadline".into()));
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}
