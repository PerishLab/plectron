#![cfg(test)]

use super::*;
use std::time::Duration;

#[test]
fn fallback() {
    let timing = Timing::timer();
    let mut seen = 0;
    let now = Instant::now();
    assert!(!timing.native());
    assert!(timing.pulse(&mut seen, Pacing::Continuous).is_some());
    assert!(timing.pulse(&mut seen, Pacing::Continuous).is_some());
    assert!(timing.pulse(&mut seen, Pacing::Idle).is_none());
    assert!(timing.pulse(&mut seen, Pacing::At(now)).is_some());
    let later = Pacing::At(now + Duration::from_secs(60));
    assert!(timing.pulse(&mut seen, later).is_none());
}

#[test]
fn distinct() {
    let timing = Timing {
        present: Some(Source { eid: 7, window: 1 }),
        pending: Cell::new(true),
        latest: Cell::new(None),
        vblank: Cell::new(None),
        period: Cell::new(None),
    };
    let mut seen = 0;
    assert!(timing.pulse(&mut seen, Pacing::Continuous).is_none());
    let mut event = CompleteNotifyEvent {
        response_type: 35,
        extension: 0,
        sequence: 0,
        length: 0,
        event_type: 1,
        kind: CompleteKind::NOTIFY_MSC,
        mode: present::CompleteMode::COPY,
        event: 8,
        window: 1,
        serial: 0,
        ust: 0,
        msc: 5,
    };
    assert!(!timing.observe(&event));
    event.event = 7;
    assert!(timing.observe(&event));
    assert!(!timing.pending.get());
    assert!(timing.pulse(&mut seen, Pacing::Continuous).is_some());
    assert_eq!(seen, 5);
    assert!(timing.pulse(&mut seen, Pacing::Continuous).is_none());
    event.msc = 6;
    assert!(timing.observe(&event));
    assert!(timing.pulse(&mut seen, Pacing::Idle).is_none());
    assert_eq!(seen, 6);
    assert!(timing.pulse(&mut seen, Pacing::Continuous).is_none());
    event.msc = 7;
    assert!(timing.observe(&event));
    assert!(
        timing
            .pulse(&mut seen, Pacing::At(Instant::now()))
            .is_some()
    );
}

#[test]
fn predict() {
    let timing = Timing::timer();
    let observed = Instant::now() + Duration::from_secs(10);
    let at = |timing: &Timing| timing.latest.get().map(|(_, at)| at).unwrap();
    timing.settle(10, 5_000_000, 5_000_400, observed);
    assert_eq!(at(&timing), observed);
    timing.settle(12, 5_033_334, 5_033_534, observed);
    let vsync = observed - Duration::from_micros(200);
    assert_eq!(at(&timing), vsync + Duration::from_micros(16_667));
    let predicted = at(&timing);
    timing.settle(13, 0, 5_050_000, observed);
    assert_eq!(at(&timing), predicted);
    let later = observed + Duration::from_millis(40);
    timing.settle(14, 0, 5_090_000, later);
    assert_eq!(at(&timing), later);
    timing.settle(15, 5_100_000, 7_000_000, later);
    assert_eq!(at(&timing), later);
    timing.settle(16, 5_116_667, 5_116_667, later + Duration::from_millis(1));
    assert_eq!(
        at(&timing),
        later + Duration::from_millis(1) + Duration::from_micros(16_667)
    );
    assert_eq!(interval((16, 5_116_667), 16, 5_120_000), None);
    assert_eq!(interval((16, 5_116_667), 17, 5_116_900), None);
    assert_eq!(vblank(9, 5, observed), None);
    let now = monotonic();
    assert!(now > 0);
    assert!(vblank(now, now, observed).is_some());
}
