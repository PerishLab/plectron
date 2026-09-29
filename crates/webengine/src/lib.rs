mod addons;
pub use addons::Bindings;
pub use addons::Web;
mod display;
pub mod keyboard;
mod loading;
mod page;
mod worker;
pub use display::{
    Compositing, Display, Disposition, Focus, Hit, MappingWork, ScrollCause, ScrollChange,
    ScrollEdge, ScrollPosition, ScrollWork,
};
pub use worker::{Action, Execution, Job, Kind, Phase, Worker};
pub mod resource;
mod snapshot;

pub use addons::event::record::{
    Action as Effect, Delivery, Origin, Phase as Propagation, Record as Dispatch,
};
pub use domengine::ime;
pub use domengine::{Direction, Edit, Selection};
pub use jsengine::Fault;
pub use page::{Collection, Computation, Loading, Milestones, Page, Relevance, State, Timing};
pub use snapshot::{Node, Snapshot, wire};

fn error(error: impl std::fmt::Display) -> Fault {
    Fault(error.to_string())
}

pub use page::operation;
