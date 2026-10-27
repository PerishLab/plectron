#![cfg(test)]

use crate::linux::{Desktop, Error, connection::Connection};
use crate::shared::{Display as _, Extent as Size, Input as _, Unit, Window as Options};
use graphics::shared::{Extent, Frame, Pacing, Scene, State as Completion, Visual};
use std::time::{Duration, Instant};

fn frame() -> Frame {
    let extent = Extent {
        width: 64,
        height: 32,
    };
    Frame::new(
        Scene::new(extent, 0, Vec::new()).unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}

fn options() -> Options {
    Options {
        title: "Plectron native visibility probe".into(),
        extent: Size {
            width: 64,
            height: 32,
        },
    }
}

fn display() -> Option<Desktop<Error>> {
    if Connection::open().is_err() {
        eprintln!("skip linux::window::sighting: no reachable X11 display");
        return None;
    }
    Some(Desktop::new("cpu", |fault| fault).unwrap())
}

fn mapped(desktop: &mut Desktop<Error>, unit: Unit) {
    desktop.create(unit, &options()).unwrap();
    desktop.activate(unit).unwrap();
    for _ in 0..200 {
        desktop.poll().unwrap();
        if desktop.windows[&unit].mapped {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("window was not mapped");
}

fn linger(desktop: &mut Desktop<Error>, span: Duration) {
    let end = Instant::now() + span;
    while Instant::now() < end {
        desktop.poll().unwrap();
        desktop
            .wait(Pacing::At(Instant::now() + Duration::from_millis(10)))
            .unwrap();
    }
}

#[test]
fn server() {
    let Some(mut desktop) = display() else {
        return;
    };
    let unit = Unit::new(1);
    mapped(&mut desktop, unit);
    assert!(desktop.windows[&unit].timing.native());
    assert!(desktop.windows[&unit].present.native);
    let mut latencies = Vec::new();
    for _ in 0..30 {
        let drawn = frame();
        let started = Instant::now();
        assert_eq!(desktop.draw(unit, &drawn).unwrap(), Completion::Pending);
        let (mut produced, mut seen) = (None, None);
        while seen.is_none() && started.elapsed() < Duration::from_secs(3) {
            desktop.poll().unwrap();
            for receipt in desktop.completed() {
                assert_eq!(receipt.frame, drawn.id());
                assert!(receipt.result.is_ok() && receipt.visible.is_none());
                produced = receipt.produced;
            }
            seen = desktop.visible().into_iter().next();
            if seen.is_none() {
                desktop
                    .wait(Pacing::At(Instant::now() + Duration::from_millis(20)))
                    .unwrap();
            }
        }
        let seen = seen.expect("no visibility for presented frame");
        let produced = produced.expect("no produced receipt");
        assert_eq!((seen.unit, seen.frame, seen.drops), (unit, drawn.id(), 0));
        assert!(seen.visible + Duration::from_millis(1) >= started);
        assert!(seen.visible <= Instant::now());
        latencies.push(
            seen.visible
                .checked_duration_since(produced)
                .map(|gap| gap.as_secs_f64() * 1000.0),
        );
    }
    assert!(desktop.visible().is_empty());
    let after: Vec<f64> = latencies.iter().flatten().copied().collect();
    eprintln!(
        "linux::window::sighting::server frames=30 visible_after_produced={}/{} min_ms={:.3} max_ms={:.3} mean_ms={:.3}",
        after.len(),
        latencies.len(),
        after.iter().copied().fold(f64::INFINITY, f64::min),
        after.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        after.iter().sum::<f64>() / after.len().max(1) as f64
    );
    desktop.close(unit).unwrap();
}

#[test]
fn burst() {
    let Some(mut desktop) = display() else {
        return;
    };
    let unit = Unit::new(2);
    mapped(&mut desktop, unit);
    let image = |shade: u32| crate::shared::Image {
        extent: Size {
            width: 64,
            height: 32,
        },
        pixels: vec![shade; 64 * 32],
    };
    let window = desktop.windows.get_mut(&unit).unwrap();
    let serials: Vec<u32> = (0..6)
        .map(|shade| {
            window
                .install(image(shade))
                .unwrap()
                .expect("present serial")
        })
        .collect();
    for (index, serial) in serials.iter().enumerate() {
        window.sightings.watch(*serial, index as u64);
    }
    let end = Instant::now() + Duration::from_secs(3);
    while desktop.windows[&unit].sightings.awaiting() > 0 && Instant::now() < end {
        linger(&mut desktop, Duration::from_millis(20));
    }
    let window = desktop.windows.get_mut(&unit).unwrap();
    assert_eq!(window.sightings.awaiting(), 0);
    let frames: Vec<u64> = window
        .sighted()
        .into_iter()
        .map(|(frame, _)| frame)
        .collect();
    eprintln!("linux::window::sighting::burst presented=6 visible_frames={frames:?}");
    assert!(frames.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(
        frames.contains(&5),
        "the last presentation must reach the screen"
    );
    desktop.close(unit).unwrap();
}

#[test]
fn hidden() {
    let Some(mut desktop) = display() else {
        return;
    };
    let unit = Unit::new(3);
    desktop.create(unit, &options()).unwrap();
    let drawn = frame();
    assert_eq!(desktop.draw(unit, &drawn).unwrap(), Completion::Pending);
    let mut produced = false;
    for _ in 0..100 {
        desktop.poll().unwrap();
        produced |= desktop
            .completed()
            .iter()
            .any(|receipt| receipt.produced.is_some());
        if produced {
            break;
        }
        desktop
            .wait(Pacing::At(Instant::now() + Duration::from_millis(10)))
            .unwrap();
    }
    assert!(produced);
    linger(&mut desktop, Duration::from_millis(250));
    assert!(desktop.visible().is_empty());
    assert_eq!(desktop.windows[&unit].sightings.awaiting(), 0);
    desktop.close(unit).unwrap();
}

#[test]
fn direct() {
    let Some(mut desktop) = display() else {
        return;
    };
    let unit = Unit::new(4);
    mapped(&mut desktop, unit);
    desktop.windows.get_mut(&unit).unwrap().present.native = false;
    let drawn = frame();
    assert_eq!(desktop.draw(unit, &drawn).unwrap(), Completion::Pending);
    let mut produced = false;
    for _ in 0..100 {
        desktop.poll().unwrap();
        produced |= desktop
            .completed()
            .iter()
            .any(|receipt| receipt.produced.is_some());
        if produced {
            break;
        }
        desktop
            .wait(Pacing::At(Instant::now() + Duration::from_millis(10)))
            .unwrap();
    }
    assert!(produced);
    linger(&mut desktop, Duration::from_millis(250));
    assert!(desktop.visible().is_empty());
    desktop.close(unit).unwrap();
}
