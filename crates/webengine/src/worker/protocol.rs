use crate::page::Publication;
use crate::{Fault, Hit};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
pub(super) fn identifier() -> Result<u64, Fault> {
    SEQUENCE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| Fault("content identifiers exhausted".into()))
}
pub enum Action {
    Demand(graphics::shared::Visual),
    Geometry(String),
    Cache {
        id: u64,
        command: shared::cache::Command,
    },
    Evaluate(String),
    Collect,
    Dispatch(Hit),
    Transition {
        from: Option<Hit>,
        to: Option<Hit>,
    },
    Receive(String),
    Edit(crate::Edit),
    Input(crate::keyboard::Input),
    Composition(crate::ime::Input),
}
impl Action {
    pub(super) fn kind(&self) -> Kind {
        match self {
            Self::Demand(_) => Kind::Demand,
            Self::Geometry(_) => Kind::Geometry,
            Self::Cache { .. } => Kind::Cache,
            Self::Evaluate(_) => Kind::Evaluate,
            Self::Collect => Kind::Collect,
            Self::Dispatch(_) => Kind::Dispatch,
            Self::Transition { .. } => Kind::Dispatch,
            Self::Receive(_) => Kind::Receive,
            Self::Edit(_) => Kind::Edit,
            Self::Input(_) => Kind::Input,
            Self::Composition(_) => Kind::Composition,
        }
    }
    pub(super) fn validate(&self) -> Result<(), Fault> {
        let valid = match self {
            Self::Geometry(id) => id.len() <= 1024,
            Self::Evaluate(source) => source.len() <= 65_536,
            Self::Receive(text) => text.len() <= 16_384,
            Self::Edit(edit) => return edit.validate().map_err(crate::error),
            Self::Input(input) => return crate::keyboard::validate(input),
            Self::Composition(input) => return input.validate().map_err(crate::error),
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(Fault("content message budget exhausted".into()))
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Demand,
    Geometry,
    Cache,
    Load,
    Evaluate,
    Collect,
    Dispatch,
    Receive,
    Edit,
    Input,
    Composition,
    Scroll,
    Hold,
}
pub use shared::jobs::Phase;
pub type Job = shared::jobs::Job<Kind>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Execution {
    pub jobs: Vec<Job>,
    pub fault: Option<String>,
}
pub(super) struct Task {
    pub context: observe::Transfer,
    pub id: u64,
    pub turn: Turn,
}
pub(super) enum Report {
    Update {
        publication: Box<Publication>,
        messages: Vec<String>,
        result: Result<(), String>,
    },
    Started(u64),
    Held(u64),
    Finished {
        id: u64,
        produced: std::time::Instant,
        publication: Box<Publication>,
        messages: Vec<String>,
        result: Result<String, String>,
    },
}

pub(super) enum Turn {
    Execute(Action),
    Scroll(Vec<crate::display::Change>, graphics::shared::Visual),
    Hold(shared::gate::Ticket),
}
impl Turn {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Execute(action) => action.kind(),
            Self::Scroll(_, _) => Kind::Scroll,
            Self::Hold(_) => Kind::Hold,
        }
    }
}
