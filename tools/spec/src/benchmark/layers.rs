use graphics::metal::{Metal, pool::Policy};
use graphics::shared::{
    Backend, Cpu, Extent, Frame, Item, Output, Pass, Rect, Scene, Space, State, Visual,
};
use serde::Serialize;
use std::time::{Duration, Instant};

pub type Benchmark = crate::benchmark::Envelope<crate::benchmark::Device, Sample>;
#[derive(Default, Serialize)]
pub struct Sample {
    scenario: String,
    pooled: bool,
    repetition: usize,
    iterations: usize,
    encoding: u64,
    gpu: u64,
    completed: usize,
    allocations: u64,
    allocated: u64,
    reuses: u64,
    pixels: u64,
    full: u64,
    retained: usize,
    maximum: u32,
    checksum: u64,
}
pub fn run() -> crate::Result<Benchmark> {
    measure().map_err(|fault| fault.to_string())
}
fn measure() -> Result<Benchmark, graphics::shared::Fault> {
    let mut metal = Metal::new()?;
    let mut samples: Vec<Sample> = Vec::new();
    for repetition in 0..3 {
        for scenario in ["small", "nested", "resize"] {
            for index in 0..2 {
                let sample = sample(
                    &mut metal,
                    scenario,
                    repetition,
                    (index + repetition) % 2 == 0,
                )?;
                record(&mut samples, sample)?;
            }
        }
    }
    Ok(Benchmark::device("nanoseconds", metal.name(), samples))
}
fn sample(
    metal: &mut Metal,
    scenario: &str,
    repetition: usize,
    pooled: bool,
) -> Result<Sample, graphics::shared::Fault> {
    metal.pool().clear()?;
    metal.pool().configure(Policy {
        entries: if pooled { 32 } else { 0 },
        bytes: 33_554_432,
    })?;
    let passes = [frame(scenario, 0)?, frame(scenario, 1)?];
    let mut report = Sample {
        scenario: scenario.into(),
        pooled,
        repetition,
        iterations: 8,
        ..Sample::default()
    };
    let mut before = metal.pool().inspect();
    for iteration in 0..10 {
        let pass = &passes[iteration % 2];
        let start = Instant::now();
        let mut work = metal.submit(pass)?;
        let encoding = super::elapsed(start);
        wait(metal, &mut work)?;
        if iteration < 2 {
            before = metal.pool().inspect();
            continue;
        }
        report.encoding += encoding;
        if let Some(duration) = work.duration() {
            report.gpu += duration.as_nanos() as u64;
            report.completed += 1;
        }
        report.full += pass.extent().area()? as u64 * if scenario == "nested" { 8 } else { 4 };
        if iteration >= 8 {
            verify((metal, &work), pass, &mut report)?;
        }
    }
    let after = metal.pool().inspect();
    report.allocations = after.allocations - before.allocations;
    report.allocated = after.allocated - before.allocated;
    report.reuses = after.reuses - before.reuses;
    report.pixels = after.pixels - before.pixels;
    report.retained = after.idle.bytes;
    Ok(report)
}
fn frame(scenario: &str, variant: usize) -> Result<Pass, graphics::shared::Fault> {
    let extent = Extent {
        width: 1024,
        height: 512,
    };
    let size = if scenario == "resize" {
        48 + variant * 24
    } else {
        48
    };
    let mut items = Vec::new();
    for index in 0..4 {
        let fill = Item::Fill {
            rect: Rect {
                x: 0,
                y: 0,
                width: size,
                height: size,
            },
            color: Some(0x114477 + index as u32 * 0x221100),
        };
        let children = if scenario == "nested" {
            vec![group(index as u64 + 5, [7, 11], vec![fill])]
        } else {
            vec![fill]
        };
        items.push(group(
            index as u64 + 1,
            [31 + index as i64 * 197, 67],
            children,
        ));
    }
    let frame = Frame::new(Scene::new(extent, 0, items)?, Visual::new(extent))?;
    frame.prepare(Output::new(extent, 1.0)?)
}
fn group(id: u64, translation: [i64; 2], items: Vec<Item>) -> Item {
    Item::Group {
        space: Space {
            id,
            translation,
            clip: None,
            scroll: None,
        },
        opacity: 163,
        items: items.into(),
    }
}
fn verify(
    hardware: (&mut Metal, &graphics::metal::Work),
    pass: &Pass,
    report: &mut Sample,
) -> Result<(), graphics::shared::Fault> {
    let expected = Cpu.submit(pass)?;
    let actual = hardware.0.read(hardware.1)?;
    if actual.extent != expected.extent || actual.pixels.len() != expected.pixels.len() {
        return Err(graphics::shared::Fault(
            "composition extent mismatch".into(),
        ));
    }
    for (a, b) in actual.pixels.iter().zip(&expected.pixels) {
        let maximum = [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255));
        report.maximum = report.maximum.max(*maximum.iter().max().unwrap());
        report.checksum = report.checksum.wrapping_mul(1099511628211) ^ u64::from(*a);
    }
    if report.maximum > 2 {
        return Err(graphics::shared::Fault(
            "composition differs from CPU".into(),
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
            return Err(graphics::shared::Fault("composition GPU deadline".into()));
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn record(samples: &mut Vec<Sample>, sample: Sample) -> Result<(), graphics::shared::Fault> {
    if samples.iter().any(|previous| {
        previous.scenario == sample.scenario && previous.checksum != sample.checksum
    }) {
        return Err(graphics::shared::Fault(
            "pooled composition checksum mismatch".into(),
        ));
    }
    samples.push(sample);
    Ok(())
}
