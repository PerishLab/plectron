use graphics::metal::{Metal, Resource};
use graphics::shared::{Backend, Cpu, Evaluation, Output, Preparation, Rasterization};
use serde::Serialize;
use shared::cache::Command;
use std::time::Instant;

pub type Benchmark = crate::benchmark::Envelope<crate::benchmark::Device, Sample>;
#[derive(Default, Serialize)]
pub struct Sample {
    scenario: String,
    mode: String,
    repetition: usize,
    preparation: u64,
    generation: u64,
    encoding: u64,
    gpu: u64,
    completed: usize,
    uploads: u64,
    uploaded: u64,
    builds: Vec<u64>,
    retained: usize,
    residency: usize,
    maximum: u32,
    coordinate: u32,
    changed: usize,
    checksum: u64,
}
pub fn run() -> crate::Result<Benchmark> {
    measure().map_err(|fault| fault.to_string())
}
fn measure() -> Result<Benchmark, graphics::shared::Fault> {
    let mut metal = Metal::new()?;
    let mut samples: Vec<Sample> = Vec::new();
    for repetition in 0..3 {
        for scenario in ["translate", "scroll", "phase", "scale", "color", "edit"] {
            for index in 0..3 {
                let mode = ["global", "local", "fresh"][(repetition + index) % 3];
                let sample = sample(&mut metal, (scenario, mode), repetition)?;
                record(&mut samples, sample)?;
            }
        }
    }
    Ok(Benchmark::device("nanoseconds", metal.name(), samples))
}
fn preparation(mode: &str) -> Preparation {
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    preparation.rasterization(if mode == "global" {
        Rasterization::Strips
    } else {
        Rasterization::Local
    });
    preparation
}
fn sample(
    metal: &mut Metal,
    names: (&str, &str),
    repetition: usize,
) -> Result<Sample, graphics::shared::Fault> {
    let (scenario, mode) = names;
    let mut preparation = preparation(mode);
    super::configure(metal, &mut preparation, mode)?;
    let mut report = Sample {
        scenario: scenario.into(),
        mode: mode.into(),
        repetition,
        ..Sample::default()
    };
    let mut before = preparation.rasterizer().report();
    let mut residency = metal.inspect(Resource::Coverages);
    for step in 0..10 {
        let frame = super::workload::frame(scenario, step)?;
        let scale = if scenario == "scale" {
            1.0 + step as f64 / 32.0
        } else {
            1.0
        };
        let output = Output::new(frame.extent(), scale)?;
        let start = Instant::now();
        let pass = preparation.prepare(&frame, output)?;
        let planning = super::super::elapsed(start);
        let start = Instant::now();
        let mut work = metal.submit(&pass)?;
        let encoding = super::super::elapsed(start);
        super::wait(metal, &mut work)?;
        if step == 0 {
            before = preparation.rasterizer().report();
            residency = metal.inspect(Resource::Coverages);
            continue;
        }
        report.preparation += planning;
        report.encoding += encoding;
        if let Some(duration) = work.duration() {
            report.gpu += duration.as_nanos() as u64;
            report.completed += 1;
        }
        let after = preparation.rasterizer().report();
        report.builds.push(after.builds - before.builds);
        report.generation += after.generation - before.generation;
        before = after;
        verify((metal, &work), (&frame, &pass), &mut report)?;
    }
    report.uploads = metal.inspect(Resource::Coverages).uploads - residency.uploads;
    report.uploaded = metal.inspect(Resource::Coverages).bytes - residency.bytes;
    report.retained = preparation.rasterizer().inspect().stats.bytes;
    report.residency = metal.resources(Resource::Coverages).stats.bytes;
    Ok(report)
}
fn verify(
    hardware: (&mut Metal, &graphics::metal::Work),
    data: (&graphics::shared::Frame, &graphics::shared::Pass),
    report: &mut Sample,
) -> Result<(), graphics::shared::Fault> {
    let expected = Cpu.submit(data.1)?;
    let actual = hardware.0.read(hardware.1)?;
    let mut fresh = preparation(&report.mode);
    fresh.command(Command::Destroy)?;
    fresh.rasterizer().command(Command::Destroy)?;
    let rebuilt = Cpu.submit(&fresh.prepare(data.0, data.1.output())?)?;
    if expected != rebuilt {
        return Err(graphics::shared::Fault(
            "motion cache changes output".into(),
        ));
    }
    report.maximum = report.maximum.max(difference(&expected, &actual));
    if report.maximum > 1 {
        return Err(graphics::shared::Fault(
            "motion Metal differs from CPU".into(),
        ));
    }
    for pixel in &expected.pixels {
        report.checksum = report.checksum.wrapping_mul(1099511628211) ^ u64::from(*pixel);
    }
    if report.builds.len() >= 8 {
        let mut direct = Preparation::default();
        direct.evaluation(Evaluation::Quadratic);
        let global = Cpu.submit(&direct.prepare(data.0, data.1.output())?)?;
        report.coordinate = report.coordinate.max(difference(&expected, &global));
        report.changed += expected
            .pixels
            .iter()
            .zip(&global.pixels)
            .filter(|(a, b)| a != b)
            .count();
        if report.mode == "global" && expected != global {
            return Err(graphics::shared::Fault(
                "global strips differ from direct evaluation".into(),
            ));
        }
    }
    Ok(())
}
fn difference(a: &graphics::shared::Image, b: &graphics::shared::Image) -> u32 {
    assert_eq!(a.extent, b.extent);
    a.pixels
        .iter()
        .zip(&b.pixels)
        .flat_map(|(a, b)| {
            [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255))
        })
        .max()
        .unwrap_or(0)
}

fn record(samples: &mut Vec<Sample>, sample: Sample) -> Result<(), graphics::shared::Fault> {
    if samples.iter().any(|previous| {
        previous.scenario == sample.scenario
            && (previous.mode == "global") == (sample.mode == "global")
            && previous.checksum != sample.checksum
    }) {
        return Err(graphics::shared::Fault("motion checksum mismatch".into()));
    }
    samples.push(sample);
    Ok(())
}
