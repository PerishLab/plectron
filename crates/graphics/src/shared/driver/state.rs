use crate::shared::{Fault, Submission, Target};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Unobserved,
    Pending,
    Dropped,
    Visible(Instant),
}

#[derive(Debug, Clone, Copy)]
pub struct Pulse {
    target: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pacing {
    Idle,
    Continuous,
    At(Instant),
}

impl Pacing {
    pub fn deadline(self) -> Option<Instant> {
        match self {
            Self::At(deadline) => Some(deadline),
            _ => None,
        }
    }

    pub fn combine(self, other: Self) -> Self {
        match (self, other) {
            (Self::Continuous, _) | (_, Self::Continuous) => Self::Continuous,
            (Self::At(left), Self::At(right)) => Self::At(left.min(right)),
            (Self::At(deadline), Self::Idle) | (Self::Idle, Self::At(deadline)) => {
                Self::At(deadline)
            }
            (Self::Idle, Self::Idle) => Self::Idle,
        }
    }
}

impl Pulse {
    pub fn new(target: Instant) -> Self {
        Self { target }
    }

    pub fn now() -> Self {
        Self::new(Instant::now())
    }

    pub fn target(self) -> Instant {
        self.target
    }
}

pub struct Receipt {
    pub target: Target,
    pub frame: u64,
    pub produced: Option<Instant>,
    pub visible: Option<Instant>,
    pub drops: u64,
    pub result: Result<(), Fault>,
}

pub struct Sighting {
    pub target: Target,
    pub frame: u64,
    pub visible: Option<Instant>,
    pub drops: u64,
}

pub(super) struct Flight {
    pub frame: u64,
    pub ticket: Submission,
    pub presenting: bool,
    pub complete: bool,
    pub produced: Option<Instant>,
    pub drops: u64,
    pub reported: bool,
    pub started: Instant,
}

pub(super) struct Update {
    pub receipt: Option<Receipt>,
    pub visible: Option<Sighting>,
    pub finished: bool,
}

impl Flight {
    pub fn new(frame: u64, ticket: Submission, started: Instant) -> Self {
        Self {
            frame,
            ticket,
            presenting: false,
            complete: false,
            produced: None,
            drops: 0,
            reported: false,
            started,
        }
    }

    pub fn initial(&mut self, target: Target) -> Update {
        self.reported = true;
        Update {
            receipt: Some(self.receipt(target, None, Ok(()))),
            visible: None,
            finished: false,
        }
    }

    pub fn terminal(&mut self, target: Target, result: Result<(), Fault>) -> Update {
        self.reported = true;
        Update {
            receipt: Some(self.receipt(target, None, result)),
            visible: None,
            finished: true,
        }
    }

    fn receipt(
        &self,
        target: Target,
        visible: Option<Instant>,
        result: Result<(), Fault>,
    ) -> Receipt {
        Receipt {
            target,
            frame: self.frame,
            produced: self.produced,
            visible,
            drops: self.drops,
            result,
        }
    }
}

impl Update {
    pub fn visible(mut self, visible: Instant) -> Self {
        self.receipt.as_mut().expect("terminal receipt").visible = Some(visible);
        self
    }
}
