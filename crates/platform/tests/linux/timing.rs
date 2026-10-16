#![cfg(test)]

use super::*;

#[test]
fn fallback() {
    let timing = Timing::timer();
    let mut seen = 0;
    assert!(!timing.native());
    assert!(timing.pulse(&mut seen).is_some());
    assert!(timing.pulse(&mut seen).is_some());
}

#[test]
fn distinct() {
    let timing = Timing {
        present: Some(Source { eid: 7, window: 1 }),
        pending: Cell::new(true),
        latest: Cell::new(None),
    };
    let mut seen = 0;
    assert!(timing.pulse(&mut seen).is_none());
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
    assert!(timing.pulse(&mut seen).is_some());
    assert_eq!(seen, 5);
    assert!(timing.pulse(&mut seen).is_none());
}
