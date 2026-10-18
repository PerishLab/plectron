#![cfg(test)]

use super::*;
use crate::shared::{Button, Point};

fn pointer(x: i64, phase: Phase) -> Event {
    Event::Pointer {
        position: Some(Point { x, y: 0 }),
        phase,
    }
}

#[test]
fn coalesce() {
    let queue = Queue::default();
    assert!(!queue.pending());
    queue.push(pointer(1, Phase::Move)).unwrap();
    queue.push(pointer(2, Phase::Move)).unwrap();
    queue
        .push(pointer(3, Phase::Down(Button::Primary)))
        .unwrap();
    queue.push(pointer(4, Phase::Move)).unwrap();
    queue.push(pointer(5, Phase::Move)).unwrap();
    assert!(queue.pending());
    assert_eq!(
        queue.drain(),
        vec![
            pointer(2, Phase::Move),
            pointer(3, Phase::Down(Button::Primary)),
            pointer(5, Phase::Move),
        ]
    );
    assert!(!queue.pending());
}

#[test]
fn bound() {
    let queue = Queue::default();
    for x in 0..512 {
        queue
            .push(pointer(x, Phase::Down(Button::Primary)))
            .unwrap();
    }
    assert_eq!(queue.push(pointer(512, Phase::Move)), Err(Exhausted));
    let mut drained = 0;
    while queue.pending() {
        let batch = queue.drain();
        assert!(batch.len() <= 16);
        drained += batch.len();
    }
    assert_eq!(drained, 512);
}

#[test]
fn close() {
    let queue = Queue::default();
    queue.push(pointer(1, Phase::Move)).unwrap();
    assert!(!queue.closed());
    queue.close();
    assert!(queue.closed());
    assert!(!queue.pending());
    queue.push(pointer(2, Phase::Move)).unwrap();
    assert!(queue.drain().is_empty());
}
