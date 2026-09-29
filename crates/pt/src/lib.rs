mod bundle;
mod component;
pub mod project;

pub use bundle::{bundle, script};
pub use component::compile;

#[derive(Debug)]
pub struct Fault(pub String);

impl std::fmt::Display for Fault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for Fault {}

pub struct Artifact {
    pub javascript: String,
}
