pub(super) mod workload;
use graphics::metal::Metal;
use graphics::shared::{Backend, Layer, Output, Pass, Primitive, State};
use serde::Serialize;
use std::time::{Duration, Instant};

pub type Benchmark = crate::benchmark::Envelope<crate::benchmark::Device, Sample>;
#[derive(Default, Serialize)]
pub struct Sample {
    scenario: String,
    mode: String,
    hits: u64,
    uploads: u64,
    uploaded: u64,
    draws: u64,
    paths: u64,
    overlaps: u64,
    retained: Retention,
    repetition: usize,
    iterations: usize,
    preparation: u64,
    encoding: u64,
    gpu: u64,
    completed: usize,
    cpu: u64,
    maximum: u32,
    deviation: u32,
    changed: usize,
    checksum: u64,
    geometry: Geometry,
}
#[derive(Default, Serialize)]
struct Geometry {
    paths: usize,
    edges: usize,
    curves: usize,
    bytes: usize,
    visits: u64,
    linear: u64,
    indices: usize,
}
#[derive(Default, Serialize)]
struct Retention {
    cpu: usize,
    gpu: usize,
}
pub fn run() -> crate::Result<Benchmark> {
    measure().map_err(|fault| fault.to_string())
}
fn measure() -> Result<Benchmark, graphics::shared::Fault> {
    let mut metal = Metal::new()?;
    let mut samples = Vec::new();
    let scenarios = ["dense", "large", "repeat", "scale"];
    for repetition in 0..3 {
        for index in 0..4 {
            for offset in 0..5 {
                record(
                    &mut samples,
                    sample(
                        &mut metal,
                        scenarios[(repetition + index) % 4],
                        repetition,
                        ["fresh", "prepared", "separate", "resident", "curves"]
                            [(repetition + offset) % 5],
                    )?,
                )?;
            }
        }
    }
    Ok(Benchmark::device("nanoseconds", metal.name(), samples))
}
fn record(samples: &mut Vec<Sample>, sample: Sample) -> Result<(), graphics::shared::Fault> {
    if samples.iter().any(|previous| {
        previous.scenario == sample.scenario
            && (previous.mode == "curves") == (sample.mode == "curves")
            && previous.checksum != sample.checksum
    }) {
        return Err(graphics::shared::Fault(
            "cached vector output differs from fresh output".into(),
        ));
    }
    samples.push(sample);
    Ok(())
}
fn sample(
    metal: &mut Metal,
    scenario: &str,
    repetition: usize,
    mode: &str,
) -> Result<Sample, graphics::shared::Fault> {
    let frame = workload::frame(scenario)?;
    metal.batching(if mode == "separate" { 1 } else { 128 })?;
    let mut preparation = graphics::shared::Preparation::default();
    if mode == "curves" {
        preparation.evaluation(graphics::shared::Evaluation::Quadratic);
    }
    metal.cache(
        graphics::metal::Resource::Paths,
        shared::cache::Command::Clear,
    )?;
    let mut policy = metal.resources(graphics::metal::Resource::Paths).active;
    policy.read = matches!(mode, "resident" | "separate" | "curves");
    policy.write = policy.read;
    metal.cache(
        graphics::metal::Resource::Paths,
        shared::cache::Command::Configure { policy },
    )?;
    let mut before = metal.inspect(graphics::metal::Resource::Paths);
    let mut hits = 0;
    let mut batches = metal.batches();
    let mut sample = Sample {
        scenario: scenario.into(),
        mode: mode.into(),
        repetition,
        iterations: 8,
        ..Sample::default()
    };
    for iteration in 0..10 {
        let scale = if scenario == "scale" && iteration % 2 == 0 {
            2.0
        } else {
            1.0
        };
        let output = Output::new(frame.extent(), scale)?;
        let clock = Instant::now();
        let pass = if mode == "fresh" {
            frame.prepare(output)?
        } else {
            preparation.prepare(&frame, output)?
        };
        let planning = super::elapsed(clock);
        let clock = Instant::now();
        let mut work = metal.submit(&pass)?;
        let encoding = super::elapsed(clock);
        wait(metal, &mut work)?;
        if iteration < 2 {
            before = metal.inspect(graphics::metal::Resource::Paths);
            hits = preparation.inspect().stats.hits;
            batches = metal.batches();
            continue;
        }
        sample.preparation += planning;
        sample.encoding += encoding;
        if let Some(duration) = work.duration() {
            sample.gpu += duration.as_nanos() as u64;
            sample.completed += 1;
        }
        geometry(pass.layer(), &mut sample.geometry);
        if iteration >= 8 {
            verify((metal, &work), (&pass, &frame), &mut sample)?;
        }
    }
    let after = metal.inspect(graphics::metal::Resource::Paths);
    sample.draws = metal.batches().calls - batches.calls;
    sample.paths = metal.batches().paths - batches.paths;
    sample.overlaps = metal.batches().overlaps - batches.overlaps;
    sample.uploads = after.uploads - before.uploads;
    sample.uploaded = after.bytes - before.bytes;
    sample.hits = preparation.inspect().stats.hits - hits;
    sample.retained = Retention {
        cpu: preparation.inspect().stats.bytes,
        gpu: metal
            .resources(graphics::metal::Resource::Paths)
            .stats
            .bytes,
    };
    Ok(sample)
}
fn geometry(layer: &Layer, report: &mut Geometry) {
    for primitive in &layer.primitives {
        match primitive {
            Primitive::Path(coverage, _) => {
                report.paths += 1;
                report.edges += coverage.edges().len();
                report.curves += coverage.curves().len();
                report.bytes +=
                    std::mem::size_of_val(coverage.edges()) + coverage.curves().len() * 32;
                report.indices += std::mem::size_of_val(coverage.bands().data());
                report.visits += coverage.visits();
                report.linear += (coverage.bounds.width
                    * coverage.bounds.height
                    * (coverage.edges().len() + coverage.curves().len())
                    * 16) as u64;
            }
            Primitive::Group(group) => geometry(group, report),
            _ => (),
        }
    }
}
fn verify(
    hardware: (&mut Metal, &graphics::metal::Work),
    data: (&Pass, &graphics::shared::Frame),
    sample: &mut Sample,
) -> Result<(), graphics::shared::Fault> {
    let (pass, frame) = data;
    let clock = Instant::now();
    let expected = graphics::shared::Cpu.submit(pass)?;
    sample.cpu += super::elapsed(clock);
    let actual = hardware.0.read(hardware.1)?;
    if actual.extent != expected.extent || actual.pixels.len() != expected.pixels.len() {
        return Err(graphics::shared::Fault(
            "vector benchmark output extent mismatch".into(),
        ));
    }
    for (a, b) in actual.pixels.iter().zip(&expected.pixels) {
        let difference = [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255));
        sample.maximum = sample.maximum.max(*difference.iter().max().unwrap());
        sample.checksum = sample.checksum.wrapping_mul(1099511628211) ^ u64::from(*b);
    }
    if sample.mode == "curves" {
        let linear = graphics::shared::Cpu.submit(&frame.prepare(pass.output())?)?;
        difference(&expected, &linear, sample);
    }
    if sample.maximum > 1 {
        return Err(graphics::shared::Fault(
            "vector benchmark differs from CPU".into(),
        ));
    }
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
            return Err(graphics::shared::Fault(
                "vector benchmark GPU deadline".into(),
            ));
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn difference(
    quadratic: &graphics::shared::Image,
    linear: &graphics::shared::Image,
    sample: &mut Sample,
) {
    for (a, b) in quadratic.pixels.iter().zip(&linear.pixels) {
        sample.changed += usize::from(a != b);
        let maximum = [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255));
        sample.deviation = sample.deviation.max(*maximum.iter().max().unwrap());
    }
}
