#![forbid(unsafe_code)]

mod device;
pub use device::{Button, Composition, Event, Id, Keyboard, Phase, Point, Signal};
mod ports;
pub use ports::{Backend, Console, Display, Input};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Unit(u64);

impl Unit {
    pub const fn new(id: u64) -> Self {
        Self(id)
    }

    pub const fn id(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Selection {
    pub anchor: usize,
    pub focus: usize,
}

impl Selection {
    pub const fn caret(position: usize) -> Self {
        Self {
            anchor: position,
            focus: position,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caret {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Focus {
    pub target: usize,
    pub text: String,
    pub selection: Selection,
    pub caret: Caret,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    pub width: usize,
    pub height: usize,
}

impl Extent {
    pub fn area(self) -> Result<usize, Bounds> {
        if self.width == 0 || self.height == 0 {
            return Err(Bounds("window dimensions must be positive"));
        }
        match self.width.checked_mul(self.height) {
            Some(size) if size <= 4_194_304 => Ok(size),
            _ => Err(Bounds("window exceeds pixel budget")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds(pub &'static str);

impl std::fmt::Display for Bounds {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for Bounds {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub title: String,
    pub extent: Extent,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub extent: Extent,
    pub pixels: Vec<u32>,
}

pub struct Receipt<E> {
    pub unit: Unit,
    pub frame: u64,
    pub produced: Option<std::time::Instant>,
    pub visible: Option<std::time::Instant>,
    pub drops: u64,
    pub result: Result<(), E>,
}

impl<E> Receipt<E> {
    pub fn map<F>(self, convert: impl FnOnce(E) -> F) -> Receipt<F> {
        let Receipt {
            unit,
            frame,
            produced,
            visible,
            drops,
            result,
        } = self;
        Receipt {
            unit,
            frame,
            produced,
            visible,
            drops,
            result: result.map_err(convert),
        }
    }
}

pub struct Visibility {
    pub unit: Unit,
    pub frame: u64,
    pub visible: std::time::Instant,
    pub drops: u64,
}
