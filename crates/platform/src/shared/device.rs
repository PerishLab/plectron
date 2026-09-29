use super::{Selection, Unit};
pub use shared::shortcut::Input as Keyboard;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Point {
    pub x: usize,
    pub y: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Button {
    Primary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Move,
    Down(Button),
    Up(Button),
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id(pub u64);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Composition {
    Mark {
        id: Id,
        text: String,
        selection: Selection,
    },
    Commit {
        id: Id,
        text: String,
    },
    Cancel {
        id: Id,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    Close,
    Pointer {
        position: Option<Point>,
        phase: Phase,
    },
    Scroll {
        delta: i64,
    },
    Wheel {
        position: Point,
        delta: i64,
    },
    Input(Keyboard),
    Composition(Composition),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub unit: Unit,
    pub event: Event,
}
