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
