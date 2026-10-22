#![cfg(test)]

use super::{Pointer, point};
use crate::shared::{Button, Event, Phase, Point};

const OUT: Event = Event::Pointer {
    position: None,
    phase: Phase::Move,
};

fn at(x: i64, y: i64) -> Point {
    Point { x, y }
}

fn pointer(x: i64, y: i64, phase: Phase) -> Event {
    Event::Pointer {
        position: Some(at(x, y)),
        phase,
    }
}

#[test]
fn signed() {
    assert_eq!(point(-0.5, 3.75), at(-1, 3));
    assert_eq!(point(-30.0, -20.25), at(-30, -21));
    assert_eq!(point(12.0, 0.0), at(12, 0));
}

#[test]
fn hover() {
    let mut state = Pointer::default();
    let mut events = Vec::new();
    state.enter(at(4, 5), &mut events);
    state.moved(at(6, 7), &mut events);
    state.exit(&mut events);
    state.exit(&mut events);
    state.enter(at(0, 9), &mut events);
    assert_eq!(
        events,
        [
            pointer(4, 5, Phase::Move),
            pointer(6, 7, Phase::Move),
            OUT,
            pointer(0, 9, Phase::Move),
        ]
    );
}

#[test]
fn outside() {
    let mut state = Pointer::default();
    let mut events = Vec::new();
    state.down(at(10, 20), &mut events);
    state.moved(at(-30, -20), &mut events);
    state.exit(&mut events);
    state.up(at(-30, -20), true, &mut events);
    state.exit(&mut events);
    assert_eq!(
        events,
        [
            pointer(10, 20, Phase::Down(Button::Primary)),
            pointer(-30, -20, Phase::Move),
            pointer(-30, -20, Phase::Up(Button::Primary)),
            OUT,
        ]
    );
}

#[test]
fn back() {
    let mut state = Pointer::default();
    let mut events = Vec::new();
    state.down(at(10, 20), &mut events);
    state.moved(at(-30, -20), &mut events);
    state.exit(&mut events);
    state.enter(at(0, 21), &mut events);
    state.moved(at(12, 22), &mut events);
    state.up(at(12, 22), false, &mut events);
    state.exit(&mut events);
    assert_eq!(
        events,
        [
            pointer(10, 20, Phase::Down(Button::Primary)),
            pointer(-30, -20, Phase::Move),
            pointer(12, 22, Phase::Move),
            pointer(12, 22, Phase::Up(Button::Primary)),
            OUT,
        ]
    );
}
