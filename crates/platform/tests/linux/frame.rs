#![cfg(test)]

use super::*;
use crate::linux::Desktop;
use crate::shared::{Display as _, Extent as Size, Input as _, Window as Options};
use graphics::shared::{Extent, Scene, Visual};
use std::time::Duration;

fn compute() -> Compute {
    Compute::new(shared::Notify::thread(std::thread::current())).unwrap()
}

fn frame(width: usize, height: usize) -> Frame {
    let extent = Extent { width, height };
    Frame::new(
        Scene::new(extent, 0, Vec::new()).unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}

fn output(frame: &Frame, scale: f64) -> Output {
    Output::new(frame.extent(), scale).unwrap()
}

fn wait(raster: &mut Raster, compute: &Compute) -> graphics::shared::Image {
    for _ in 0..500 {
        if let Some(image) = raster.poll(compute) {
            return image;
        }
        std::thread::park_timeout(Duration::from_millis(10));
    }
    panic!("raster job did not finish");
}

fn receipts(raster: &mut Raster) -> Vec<(u64, bool, bool)> {
    std::mem::take(&mut raster.done)
        .into_iter()
        .map(|done| (done.frame, done.produced.is_some(), done.result.is_ok()))
        .collect()
}

#[test]
fn stale() {
    let compute = compute();
    let mut raster = Raster::default();
    let (first, second) = (frame(32, 16), frame(32, 16));
    let draw = raster.draw(&first, output(&first, 1.0), &compute);
    assert_eq!(draw, Completion::Pending);
    let draw = raster.draw(&second, output(&second, 1.0), &compute);
    assert_eq!(draw, Completion::Pending);
    assert_eq!(receipts(&mut raster), vec![(first.id(), false, false)]);
    wait(&mut raster, &compute);
    assert!(raster.poll(&compute).is_none());
    raster.present(Ok(())).unwrap();
    assert_eq!(receipts(&mut raster), vec![(second.id(), true, true)]);
    let third = frame(32, 16);
    raster.draw(&third, output(&third, 1.0), &compute);
    raster.clear();
    assert_eq!(receipts(&mut raster), vec![(third.id(), false, false)]);
    assert!(raster.poll(&compute).is_none());
    assert!(raster.frame().is_none());
}

#[test]
fn resize() {
    let compute = compute();
    let mut raster = Raster::default();
    let frame = frame(32, 16);
    raster.draw(&frame, output(&frame, 1.0), &compute);
    assert!(!raster.refresh(2.0, &compute).unwrap());
    let image = wait(&mut raster, &compute);
    assert_eq!(output(&frame, 2.0).pixels(), image.extent);
    raster.present(Ok(())).unwrap();
    assert_eq!(receipts(&mut raster), vec![(frame.id(), true, true)]);
    assert!(!raster.refresh(1.0, &compute).unwrap());
    assert!(raster.pending());
    let image = wait(&mut raster, &compute);
    assert_eq!(output(&frame, 1.0).pixels(), image.extent);
    raster.present(Ok(())).unwrap();
    assert!(receipts(&mut raster).is_empty());
    assert!(raster.refresh(1.0, &compute).unwrap());
    assert!(!raster.pending());
    assert_eq!(raster.frame().map(Frame::id), Some(frame.id()));
}

#[test]
fn close() {
    let compute = compute();
    let mut raster = Raster::default();
    let first = frame(512, 512);
    raster.draw(&first, output(&first, 2.0), &compute);
    assert!(raster.pending());
    drop(raster);
    let mut raster = Raster::default();
    let second = frame(32, 16);
    raster.draw(&second, output(&second, 1.0), &compute);
    wait(&mut raster, &compute);
    raster.present(Ok(())).unwrap();
    assert_eq!(receipts(&mut raster), vec![(second.id(), true, true)]);
}

#[test]
fn produced() {
    let compute = compute();
    let mut raster = Raster::default();
    let drawn = frame(32, 16);
    raster.draw(&drawn, output(&drawn, 1.0), &compute);
    wait(&mut raster, &compute);
    let started = Instant::now();
    raster.present(Ok(())).unwrap();
    let done = raster.done.pop().unwrap();
    assert_eq!(done.frame, drawn.id());
    assert!(done.produced.is_some_and(|produced| produced >= started));
    assert!(
        done.produced
            .is_some_and(|produced| produced <= Instant::now())
    );
    let failed = frame(32, 16);
    raster.draw(&failed, output(&failed, 1.0), &compute);
    wait(&mut raster, &compute);
    raster.present(Err(Error("copy failed".into()))).unwrap();
    assert_eq!(receipts(&mut raster), vec![(failed.id(), false, false)]);
    assert!(!raster.refresh(2.0, &compute).unwrap());
    wait(&mut raster, &compute);
    assert!(raster.present(Err(Error("copy failed".into()))).is_err());
    assert!(receipts(&mut raster).is_empty());
}

#[test]
fn desktop() {
    if Connection::open().is_err() {
        eprintln!("skip linux::window::frame::desktop: no reachable X11 display");
        return;
    }
    let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
    let options = Options {
        title: "Plectron native frame probe".into(),
        extent: Size {
            width: 64,
            height: 32,
        },
    };
    let (closed, kept) = (Unit::new(1), Unit::new(2));
    desktop.create(closed, &options).unwrap();
    desktop.create(kept, &options).unwrap();
    let (first, second) = (frame(64, 32), frame(64, 32));
    assert_eq!(desktop.draw(closed, &first).unwrap(), Completion::Pending);
    desktop.close(closed).unwrap();
    assert_eq!(desktop.draw(kept, &second).unwrap(), Completion::Pending);
    let started = Instant::now();
    let mut delivered = Vec::new();
    for _ in 0..500 {
        delivered.extend(desktop.completed());
        if !delivered.is_empty() {
            break;
        }
        desktop.wait(graphics::shared::Pacing::Idle).unwrap();
    }
    assert_eq!(delivered.len(), 1);
    let receipt = &delivered[0];
    assert_eq!((receipt.unit, receipt.frame), (kept, second.id()));
    assert!(receipt.result.is_ok());
    assert!(receipt.produced.is_some_and(|produced| produced >= started));
    assert!(receipt.visible.is_none());
    assert!(desktop.completed().is_empty());
}

#[test]
fn destroy() {
    let Ok(external) = Connection::open() else {
        eprintln!("skip linux::window::frame::destroy: no reachable X11 display");
        return;
    };
    let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
    let unit = Unit::new(7);
    let options = Options {
        title: "Plectron native destroy probe".into(),
        extent: Size {
            width: 64,
            height: 32,
        },
    };
    desktop.create(unit, &options).unwrap();
    desktop.activate(unit).unwrap();
    let drawn = frame(64, 32);
    assert_eq!(desktop.draw(unit, &drawn).unwrap(), Completion::Pending);
    let xid = desktop.windows[&unit].xid;
    external.conn.destroy_window(xid).unwrap().check().unwrap();
    let (mut closed, mut delivered) = (false, Vec::new());
    for _ in 0..100 {
        closed |= desktop
            .poll()
            .unwrap()
            .iter()
            .any(|signal| signal.unit == unit && signal.event == Event::Close);
        delivered.extend(desktop.completed());
        if closed && !delivered.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(closed, "external destroy did not deliver Close");
    assert_eq!(delivered.len(), 1);
    assert_eq!(delivered[0].frame, drawn.id());
    assert!(delivered[0].result.is_err() && delivered[0].produced.is_none());
    desktop.close(unit).unwrap();
    assert!(desktop.title(unit, "closed").is_err());
}
