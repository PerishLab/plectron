mod compare;
mod driver;
mod input;
mod metrics;
mod plan;
mod storage;
mod summary;
mod text;

pub use compare::{Comparison, Rules, compare};
pub use driver::{Manifest, run};
pub use input::Document;
pub use metrics::{Group, Timing};
pub use plan::Plan;
pub use storage::{Inspection, inspect};
pub use summary::Summary;

pub fn read(path: &std::path::Path) -> crate::Result<Document> {
    Document::parse(storage::File(path).read()?)
}

pub mod observation;
