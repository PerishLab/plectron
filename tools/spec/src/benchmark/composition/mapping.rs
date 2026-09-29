use super::super::elapsed;
use crate::Result;
use graphics::shared::{Extent, Frame, Rect, Scene, Space, Spatial, Visual};
use serde::Serialize;
use std::{hint::black_box, time::Instant};

#[derive(Serialize)]
pub struct Report {
    unit: &'static str,
    architecture: &'static str,
    system: &'static str,
    debug: bool,
    samples: Vec<Sample>,
}

#[derive(Serialize)]
struct Sample {
    repetition: usize,
    spaces: usize,
    iterations: usize,
    eager: u64,
    publication: u64,
    pending: u64,
    resolution: u64,
    eagerwork: graphics::shared::Materialization,
    pendingwork: graphics::shared::Materialization,
}

struct Eager {
    elapsed: u64,
    frame: Frame,
    work: graphics::shared::Materialization,
}

struct Deferred {
    pending: u64,
    resolution: u64,
    frame: Frame,
    work: graphics::shared::Materialization,
}

pub fn run(
    platform: (&'static str, &'static str),
    spaces: usize,
    iterations: usize,
) -> Result<Report> {
    if spaces < 2 || iterations == 0 || iterations > 1_048_576 {
        return Err("mapping experiment dimensions are outside the supported range".into());
    }
    let (scene, initial) = fixture(spaces, iterations)?;
    let mut samples = Vec::with_capacity(7);
    for repetition in 0usize..7 {
        let (eager, deferred) = if repetition.is_multiple_of(2) {
            (
                consume(&initial, &scene, iterations)?,
                pending(&initial, &scene, iterations)?,
            )
        } else {
            let deferred = pending(&initial, &scene, iterations)?;
            (consume(&initial, &scene, iterations)?, deferred)
        };
        let (publication, latest) = publish(&initial, iterations);
        black_box(latest);
        let consumed = eager.work.materializations == iterations as u64
            && eager.work.queries == iterations as u64;
        let coalesced = deferred.work.materializations == 1 && deferred.work.queries == 1;
        if eager.frame.mappings() != deferred.frame.mappings() || !consumed || !coalesced {
            return Err("mapping experiment paths diverged".into());
        }
        samples.push(Sample {
            repetition,
            spaces,
            iterations,
            eager: eager.elapsed,
            publication,
            pending: deferred.pending,
            resolution: deferred.resolution,
            eagerwork: eager.work,
            pendingwork: deferred.work,
        });
    }
    Ok(Report {
        unit: "nanoseconds",
        architecture: platform.0,
        system: platform.1,
        debug: cfg!(debug_assertions),
        samples,
    })
}

fn fixture(spaces: usize, iterations: usize) -> Result<(Scene, Frame)> {
    let extent = Extent {
        width: 400,
        height: 240,
    };
    let mut nodes = Vec::with_capacity(spaces - 1);
    nodes.push(Spatial {
        space: Space {
            id: 1,
            translation: [0; 2],
            clip: Some(
                Rect {
                    x: 0,
                    y: 0,
                    width: extent.width,
                    height: extent.height,
                }
                .into(),
            ),
            scroll: Some(iterations),
        },
        opacity: 255,
        parent: 0,
        descendants: spaces - 2,
    });
    for id in 2..spaces as u64 {
        nodes.push(Spatial {
            space: Space {
                id,
                translation: [0, id as i64 % 240],
                clip: None,
                scroll: None,
            },
            opacity: 255,
            parent: 1,
            descendants: 0,
        });
    }
    let scene = Scene::project(extent, 0, nodes).map_err(error)?;
    let frame = Frame::new(scene.clone(), Visual::new(extent)).map_err(error)?;
    Ok((scene, frame))
}

fn consume(initial: &Frame, scene: &Scene, iterations: usize) -> Result<Eager> {
    let mut frame = initial.clone();
    let before = frame.materialization();
    let started = Instant::now();
    for offset in 1..=iterations {
        let visual = visual(initial, offset);
        frame = frame.revise(scene.clone(), visual).map_err(error)?;
        black_box(frame.mapping(1));
    }
    Ok(Eager {
        elapsed: elapsed(started),
        work: frame.materialization().since(before),
        frame,
    })
}

fn publish(source: &Frame, iterations: usize) -> (u64, Visual) {
    let mut latest = source.visual().clone();
    let started = Instant::now();
    for offset in 1..=iterations {
        latest = visual(source, offset);
        black_box(&latest);
    }
    (elapsed(started), latest)
}

fn pending(base: &Frame, scene: &Scene, iterations: usize) -> Result<Deferred> {
    let mut frame = base.clone();
    let before = frame.materialization();
    let started = Instant::now();
    for offset in 1..=iterations {
        frame = frame
            .revise(scene.clone(), visual(base, offset))
            .map_err(error)?;
        black_box(frame.id());
    }
    let pending = elapsed(started);
    let started = Instant::now();
    black_box(frame.mapping(1));
    Ok(Deferred {
        pending,
        resolution: elapsed(started),
        work: frame.materialization().since(before),
        frame,
    })
}

fn visual(source: &Frame, offset: usize) -> Visual {
    let mut visual = source.visual().clone();
    visual.offsets.insert(1, offset);
    visual.revision = offset as u64;
    visual
}

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
