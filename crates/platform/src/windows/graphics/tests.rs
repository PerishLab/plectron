use super::super::*;
use crate::shared::Extent;
use ::graphics::shared::{Extent as Size, Frame, Scene, State as Completion, Visual};

fn frame(width: usize, height: usize, revision: u64) -> Frame {
    let extent = Size { width, height };
    Frame::new(
        Scene::new(extent, revision, Vec::new()).unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}

#[test]
fn cadence() {
    let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
    let unit = Unit::new(1);
    let options = Options::new(
        "Plectron cadence probe",
        Extent {
            width: 64,
            height: 32,
        },
    );
    desktop.create(unit, &options).unwrap();
    desktop.activate(unit).unwrap();
    let sample = frame(64, 32, 0);
    assert_eq!(desktop.draw(unit, &sample).unwrap(), Completion::Pending);
    let started = Instant::now();
    let (mut produced, mut seen) = (None, None);
    while started.elapsed() < Duration::from_secs(3) {
        desktop.poll().unwrap();
        for receipt in desktop.completed() {
            assert_eq!(receipt.frame, sample.id());
            assert!(receipt.result.is_ok());
            produced = receipt.produced;
        }
        if let Some(item) = desktop.visible().into_iter().next() {
            seen = Some(item);
        }
        if produced.is_some() && (!desktop.timing.native() || seen.is_some()) {
            break;
        }
        desktop.wait(Pacing::Idle).unwrap();
    }
    let produced = produced.expect("no produced receipt");
    if desktop.timing.native() {
        let item = seen.expect("no DWM visibility sighting");
        assert_eq!((item.unit, item.frame, item.drops), (unit, sample.id(), 0));
        assert!(produced <= item.visible);
        assert!(item.visible <= Instant::now());
    }
    desktop.pace(unit, Pacing::Continuous).unwrap();
    let mut pulses = 0usize;
    let begin = Instant::now();
    while pulses < 3 && begin.elapsed() < Duration::from_secs(2) {
        desktop.wait(Pacing::Continuous).unwrap();
        desktop.poll().unwrap();
        if desktop.pulse(unit).unwrap().is_some() {
            pulses += 1;
        }
    }
    assert!(pulses >= 2);
    desktop.timing = timing::Timing::timer();
    assert!(desktop.pulse(unit).unwrap().is_some());
    desktop.pace(unit, Pacing::Idle).unwrap();
    assert!(desktop.pulse(unit).unwrap().is_none());
    desktop.close(unit).unwrap();
}

#[test]
fn d3d12() {
    let mut desktop = Desktop::new("d3d12", |fault| fault).unwrap();
    desktop.prepare().unwrap();
    let unit = Unit::new(1);
    let options = Options::new(
        "Plectron D3D12 probe",
        Extent {
            width: 160,
            height: 96,
        },
    );
    desktop.create(unit, &options).unwrap();
    desktop.activate(unit).unwrap();
    let first = frame(160, 96, 1);
    assert_eq!(desktop.draw(unit, &first).unwrap(), Completion::Pending);
    let (produced, seen) = awaitframe(&mut desktop, unit, first.id());
    let produced = produced.expect("first D3D12 produced receipt");
    if desktop.timing.native() {
        let item = seen.expect("first D3D12 visibility sighting");
        assert_eq!((item.unit, item.frame, item.drops), (unit, first.id(), 0));
        assert!(produced <= item.visible);
        assert!(item.visible <= Instant::now());
    }
    let second = frame(160, 96, 2);
    assert_eq!(desktop.draw(unit, &second).unwrap(), Completion::Pending);
    let (p2, s2) = awaitframe(&mut desktop, unit, second.id());
    assert!(p2.is_some());
    if desktop.timing.native() {
        let item = s2.expect("second D3D12 visibility sighting");
        assert_eq!((item.unit, item.frame), (unit, second.id()));
        assert!(p2.unwrap() <= item.visible);
    }
    let third = frame(200, 120, 3);
    assert_eq!(desktop.draw(unit, &third).unwrap(), Completion::Pending);
    let (resized, s3) = awaitframe(&mut desktop, unit, third.id());
    assert!(resized.is_some());
    if desktop.timing.native() {
        let item = s3.expect("resized D3D12 visibility sighting");
        assert_eq!((item.unit, item.frame), (unit, third.id()));
        assert!(resized.unwrap() <= item.visible);
    }
    desktop.close(unit).unwrap();
}

#[test]
fn recovery() {
    let mut desktop = Desktop::new("d3d12", |fault| fault).unwrap();
    let unit = Unit::new(1);
    let options = Options::new(
        "Plectron D3D12 recovery probe",
        Extent {
            width: 128,
            height: 64,
        },
    );
    desktop.create(unit, &options).unwrap();
    desktop.activate(unit).unwrap();
    assert!(desktop.window(unit).unwrap().accelerated());
    let sample = frame(128, 64, 0x336699);
    desktop.graphics.inject(
        unit,
        sample.clone(),
        Error("DXGI_ERROR_DEVICE_REMOVED".into()),
    );
    let started = Instant::now();
    let mut recovered = false;
    while started.elapsed() < Duration::from_secs(3) {
        desktop.poll().unwrap();
        for receipt in desktop.completed() {
            assert_eq!(receipt.frame, sample.id());
            assert!(receipt.result.is_ok());
            assert!(receipt.produced.is_some());
            recovered = true;
        }
        if recovered {
            break;
        }
        desktop.wait(Pacing::Idle).unwrap();
    }
    assert!(recovered);
    assert!(!desktop.window(unit).unwrap().accelerated());
    let next = frame(128, 64, 0x112233);
    assert_eq!(desktop.draw(unit, &next).unwrap(), Completion::Pending);
    let started = Instant::now();
    let mut finished = false;
    while started.elapsed() < Duration::from_secs(3) {
        desktop.poll().unwrap();
        for receipt in desktop.completed() {
            assert_eq!(receipt.frame, next.id());
            assert!(receipt.result.is_ok());
            finished = true;
        }
        if finished {
            break;
        }
        desktop.wait(Pacing::Idle).unwrap();
    }
    assert!(finished);
    desktop.close(unit).unwrap();
}

#[test]
fn handoff() {
    let mut desktop = Desktop::new("d3d12", |fault| fault).unwrap();
    desktop.prepare().unwrap();
    desktop.graphics.hold(true);
    let unit = Unit::new(1);
    let options = Options::new(
        "Plectron D3D12 handoff probe",
        Extent {
            width: 160,
            height: 96,
        },
    );
    desktop.create(unit, &options).unwrap();
    desktop.activate(unit).unwrap();
    let first = frame(160, 96, 1);
    assert_eq!(desktop.draw(unit, &first).unwrap(), Completion::Pending);
    assert_eq!(desktop.graphics.hedged(), 1);
    let (produced, seen) = awaitframe(&mut desktop, unit, first.id());
    let produced = produced.expect("hedged first frame receipt");
    assert!(desktop.window(unit).unwrap().accelerated());
    if desktop.timing.native() {
        let item = seen.expect("hedged first frame visibility sighting");
        assert_eq!((item.unit, item.frame, item.drops), (unit, first.id(), 0));
        assert!(produced <= item.visible);
        assert!(item.visible <= Instant::now());
    }
    desktop.graphics.hold(false);
    let started = Instant::now();
    while !desktop.graphics.ready() && started.elapsed() < Duration::from_secs(3) {
        desktop
            .wait(Pacing::At(Instant::now() + Duration::from_millis(5)))
            .unwrap();
    }
    assert!(desktop.graphics.ready());
    let second = frame(160, 96, 2);
    assert_eq!(desktop.draw(unit, &second).unwrap(), Completion::Pending);
    assert_eq!(desktop.graphics.hedged(), 1);
    let (p2, s2) = awaitframe(&mut desktop, unit, second.id());
    assert!(p2.is_some());
    if desktop.timing.native() {
        let item = s2.expect("promoted D3D12 visibility sighting");
        assert_eq!((item.unit, item.frame), (unit, second.id()));
        assert!(p2.unwrap() <= item.visible);
    }
    desktop.close(unit).unwrap();
}

#[test]
fn refusal() {
    assert!(Desktop::<Error>::new("metal", |fault| fault).is_err());
    let mut desktop = Desktop::new("cpu", |fault| fault).unwrap();
    let unit = Unit::new(1);
    let mut options = Options::new(
        "invalid\0title",
        Extent {
            width: 64,
            height: 32,
        },
    );
    assert!(desktop.create(unit, &options).is_err());
    options.title = "Plectron ownership probe".into();
    desktop.create(unit, &options).unwrap();
    assert!(desktop.create(unit, &options).is_err());
    desktop.close(unit).unwrap();
    assert!(desktop.title(unit, "closed").is_err());
    assert!(desktop.close(unit).is_err());
}

fn awaitframe(
    desktop: &mut Desktop<Error>,
    unit: Unit,
    id: u64,
) -> (Option<Instant>, Option<Visibility>) {
    let started = Instant::now();
    let (mut produced, mut seen) = (None, None);
    while started.elapsed() < Duration::from_secs(3) {
        desktop.poll().unwrap();
        collect(desktop, (unit, id), (&mut produced, &mut seen));
        if produced.is_some() && seen.is_some() {
            break;
        }
        desktop.wait(Pacing::Idle).unwrap();
    }
    (produced, seen)
}

fn collect(
    desktop: &mut Desktop<Error>,
    target: (Unit, u64),
    state: (&mut Option<Instant>, &mut Option<Visibility>),
) {
    let (unit, id) = target;
    let (produced, seen) = state;
    for receipt in desktop.completed() {
        if receipt.unit != unit || receipt.frame != id {
            continue;
        }
        assert!(receipt.result.is_ok(), "{:?}", receipt.result);
        *produced = receipt.produced;
        if let Some(visible) = receipt.visible {
            *seen = Some(Visibility {
                unit,
                frame: id,
                visible,
                drops: receipt.drops,
            });
        }
    }
    for item in desktop.visible() {
        if item.unit == unit && item.frame == id {
            *seen = Some(item);
        }
    }
}
