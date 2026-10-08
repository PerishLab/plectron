use crate::contract::Signal;
use serde::{Deserialize, Serialize};
use shared::gate::Ticket;

pub(super) enum Action {
    Startup(crate::runtime::source::Input),
    Evaluate(String),
    Dispatch(Signal),
    Hold(Ticket),
}
impl Action {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Startup(_) => Kind::Startup,
            Self::Evaluate(_) => Kind::Evaluate,
            Self::Dispatch(_) => Kind::Dispatch,
            Self::Hold(_) => Kind::Hold,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Startup,
    Evaluate,
    Dispatch,
    Hold,
}
pub use shared::jobs::Phase;
pub type Job = shared::jobs::Job<Kind>;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub activity: jsengine::Activity,
    pub performance: jsengine::jit::Report,
    pub waiting: bool,
    pub jobs: Vec<Job>,
    pub fault: Option<String>,
}
pub(super) struct Task {
    pub context: observe::Transfer,
    pub id: u64,
    pub action: Action,
}
pub(super) enum Report {
    Started(u64),
    Held(u64),
    Finished(
        u64,
        Result<String, String>,
        jsengine::Activity,
        jsengine::jit::Report,
        std::time::Instant,
    ),
}
