use super::Sample;
use crate::Result;
use graphics::metal::Metal;
use graphics::shared::{
    Backend, Extent, Frame, Ink, Item, Output, Raster, Record, Rect, Scene, Space, State, Visual,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
pub(super) fn sample(scenario: &str, repetition: usize, cached: bool) -> Result<Sample> {
    run(scenario, repetition, cached).map_err(|fault| fault.to_string())
}
fn run(
    scenario: &str,
    repetition: usize,
    cached: bool,
) -> std::result::Result<Sample, graphics::shared::Fault> {
    let mut metal = Metal::new()?;
    let mut policy = metal.resources(graphics::metal::Resource::Textures).active;
    policy.read = cached;
    policy.write = cached;
    metal.cache(
        graphics::metal::Resource::Textures,
        shared::cache::Command::Configure { policy },
    )?;
    let extent = Extent {
        width: 256,
        height: 256,
    };
    let output = Output::new(extent, 1.0)?;
    let raster = Raster::rgba(
        Extent {
            width: 64,
            height: 64,
        },
        &[32, 120, 160, 200].repeat(4096),
    )?;
    let mut sample = Sample {
        scenario: format!("gpu-{scenario}"),
        repetition,
        iterations: 16,
        cached: Some(cached),
        ..Sample::default()
    };
    let original = frame(raster.clone(), scenario, 0)?;
    let mut before = metal.inspect(graphics::metal::Resource::Textures);
    for index in 0..=16 {
        let source = if scenario == "replace" {
            Raster::rgba(raster.extent(), &[index as u8, 120, 160, 200].repeat(4096))?
        } else {
            raster.clone()
        };
        let frame = if matches!(scenario, "repeat" | "instances") {
            original.clone()
        } else {
            frame(source, scenario, index)?
        };
        let clock = Instant::now();
        let pass = frame.prepare(output)?;
        let planning = super::super::elapsed(clock);
        let clock = Instant::now();
        let mut work = metal.submit(&pass)?;
        let elapsed = super::super::elapsed(clock);
        wait(&mut metal, &mut work)?;
        match index {
            0 => {
                before = metal.inspect(graphics::metal::Resource::Textures);
                continue;
            }
            16 => verify(&mut metal, &work, &pass)?,
            _ => {}
        }
        sample.planning += planning;
        sample.elapsed += elapsed;
        if let Some(duration) = work.duration() {
            sample.gpu += duration.as_nanos() as u64;
            sample.completed += 1;
        }
    }
    let after = metal.inspect(graphics::metal::Resource::Textures);
    sample.uploads = after.uploads - before.uploads;
    sample.bytes = after.bytes - before.bytes;
    Ok(sample)
}
fn frame(
    raster: Raster,
    scenario: &str,
    index: usize,
) -> std::result::Result<Frame, graphics::shared::Fault> {
    let extent = Extent {
        width: 256,
        height: 256,
    };
    let record = Arc::new(Record::new(Arc::new(Ink::Image {
        raster,
        rect: Rect {
            x: 0,
            y: 0,
            width: 24,
            height: 24,
        },
    }))?);
    let count = if scenario == "instances" { 64 } else { 1 };
    let items = (0..count)
        .map(|index| Item::Draw {
            origin: [index % 8 * 28, index / 8 * 28],
            clip: None,
            record: record.clone(),
        })
        .collect();
    let changed = scenario == "visual" && index.is_multiple_of(2);
    let scene = Scene::new(
        extent,
        index as u64,
        vec![Item::Group {
            space: Space {
                id: 1,
                translation: [if changed { 10 } else { 0 }, 0],
                clip: None,
                scroll: None,
            },
            opacity: if changed { 128 } else { 255 },
            items,
        }],
    )?;
    Frame::new(scene, Visual::new(extent))
}
fn wait(
    metal: &mut Metal,
    work: &mut graphics::metal::Work,
) -> std::result::Result<(), graphics::shared::Fault> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match metal.poll(work)? {
            State::Complete => return Ok(()),
            State::Failed(fault) => return Err(fault),
            State::Pending => (),
        }
        if Instant::now() > deadline {
            return Err(graphics::shared::Fault("benchmark GPU deadline".into()));
        }
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn verify(
    metal: &mut Metal,
    work: &graphics::metal::Work,
    pass: &graphics::shared::Pass,
) -> std::result::Result<(), graphics::shared::Fault> {
    let expected = graphics::shared::Cpu.submit(pass)?;
    let actual = metal.read(work)?;
    if actual.pixels.iter().zip(&expected.pixels).any(|(a, b)| {
        [0, 8, 16]
            .into_iter()
            .any(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255) > 1)
    }) {
        return Err(graphics::shared::Fault(
            "GPU benchmark differs from CPU".into(),
        ));
    }
    Ok(())
}
