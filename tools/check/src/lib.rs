mod cargo;
mod change;
mod model;
mod policy;
mod runner;
pub mod types;

pub use change::{Change, Kind, Source, parse};
pub use model::{Cost, Domain, Gate, Plan};

pub type Result<T> = std::result::Result<T, String>;

pub fn plan(root: &std::path::Path, source: &Source, full: bool) -> Result<Plan> {
    let changes = source.read(root)?;
    let graph = cargo::Graph::load(root)?;
    Ok(policy::build(changes, &graph, full))
}

pub fn execute(root: &std::path::Path, plan: &mut Plan) -> Result<()> {
    runner::execute(root, plan)
}
