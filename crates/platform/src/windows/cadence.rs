use super::*;
use crate::shared::Extent;
use graphics::shared::{Extent as Size, Frame, Scene, State as Completion, Visual};

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
    let extent = Size {
        width: 64,
        height: 32,
    };
    let frame = Frame::new(
        Scene::new(extent, 0, Vec::new()).unwrap(),
        Visual::new(extent),
    )
    .unwrap();
    assert_eq!(desktop.draw(unit, &frame).unwrap(), Completion::Pending);
    let started = Instant::now();
    let (mut produced, mut seen) = (None, None);
    while started.elapsed() < Duration::from_secs(3) {
        desktop.poll().unwrap();
        for receipt in desktop.completed() {
            assert_eq!(receipt.frame, frame.id());
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
        assert_eq!((item.unit, item.frame, item.drops), (unit, frame.id(), 0));
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
