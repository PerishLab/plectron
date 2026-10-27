#![cfg(test)]

use super::*;
use std::time::Duration;
use x11rb::protocol::present::CompleteNotifyEvent;

fn event(kind: CompleteKind, mode: CompleteMode, serial: u32, ust: u64) -> CompleteNotifyEvent {
    CompleteNotifyEvent {
        response_type: 35,
        extension: 0,
        sequence: 0,
        length: 0,
        event_type: 1,
        kind,
        mode,
        event: 7,
        window: 1,
        serial,
        ust,
        msc: 3,
    }
}

fn pixmap(mode: CompleteMode, serial: u32, ust: u64) -> CompleteNotifyEvent {
    event(CompleteKind::PIXMAP, mode, serial, ust)
}

#[test]
fn settle() {
    let observed = Instant::now();
    let mut sightings = Sightings::default();
    let first = sightings.serial();
    sightings.watch(first, 10);
    sightings.settle(
        &pixmap(CompleteMode::COPY, first, 995_000),
        true,
        1_000_000,
        observed,
    );
    let seen = sightings.take();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].0, 10);
    assert_eq!(seen[0].1, observed - Duration::from_millis(5));
    assert!(sightings.take().is_empty());
    assert_eq!(sightings.awaiting(), 0);
    for (serial, mode) in [(2, CompleteMode::FLIP), (3, CompleteMode::SUBOPTIMAL_COPY)] {
        sightings.watch(serial, u64::from(serial));
        sightings.settle(&pixmap(mode, serial, 999_000), true, 1_000_000, observed);
    }
    let frames: Vec<u64> = sightings
        .take()
        .into_iter()
        .map(|(frame, _)| frame)
        .collect();
    assert_eq!(frames, vec![2, 3]);
}

#[test]
fn absent() {
    let observed = Instant::now();
    let mut sightings = Sightings::default();
    let now = 100_000_000;
    let cases = [
        (pixmap(CompleteMode::SKIP, 1, now - 1_000), true),
        (pixmap(CompleteMode::COPY, 1, 0), true),
        (pixmap(CompleteMode::COPY, 1, now + 1), true),
        (pixmap(CompleteMode::COPY, 1, now - 60_000_001), true),
        (pixmap(CompleteMode::COPY, 1, now - 1_000), false),
        (
            event(CompleteKind::NOTIFY_MSC, CompleteMode::COPY, 1, now - 1_000),
            true,
        ),
        (pixmap(CompleteMode::COPY, 9, now - 1_000), true),
    ];
    for (event, mapped) in cases {
        sightings.watch(1, 5);
        sightings.settle(&event, mapped, now, observed);
        assert!(sightings.take().is_empty(), "{:?} {mapped}", event.mode);
        sightings.awaited.clear();
    }
    sightings.watch(1, 5);
    sightings.settle(
        &pixmap(CompleteMode::SKIP, 1, now - 1_000),
        true,
        now,
        observed,
    );
    assert_eq!(sightings.awaiting(), 0);
    sightings.settle(
        &pixmap(CompleteMode::COPY, 1, now - 1_000),
        true,
        now,
        observed,
    );
    assert!(sightings.take().is_empty());
}

#[test]
fn wrap() {
    let observed = Instant::now();
    let mut sightings = Sightings {
        last: u32::MAX - 1,
        ..Sightings::default()
    };
    let serials: Vec<u32> = (0..4).map(|_| sightings.serial()).collect();
    assert_eq!(serials, vec![u32::MAX, 0, 1, 2]);
    for (index, serial) in serials.iter().enumerate() {
        sightings.watch(*serial, index as u64);
    }
    for serial in [0, u32::MAX, 2] {
        sightings.settle(
            &pixmap(CompleteMode::COPY, serial, 999_000),
            true,
            1_000_000,
            observed,
        );
    }
    let frames: Vec<u64> = sightings
        .take()
        .into_iter()
        .map(|(frame, _)| frame)
        .collect();
    assert_eq!(frames, vec![1, 0, 3]);
    assert_eq!(sightings.awaiting(), 1);
}

#[test]
fn bound() {
    let observed = Instant::now();
    let mut sightings = Sightings::default();
    for frame in 0..(AWAITED as u64 + 10) {
        let serial = sightings.serial();
        sightings.watch(serial, frame);
    }
    assert_eq!(sightings.awaiting(), AWAITED);
    sightings.settle(
        &pixmap(CompleteMode::COPY, 1, 999_000),
        true,
        1_000_000,
        observed,
    );
    assert!(sightings.take().is_empty());
    sightings.settle(
        &pixmap(CompleteMode::COPY, 11, 999_000),
        true,
        1_000_000,
        observed,
    );
    assert_eq!(sightings.take().len(), 1);
}
