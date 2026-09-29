mod bundle;
mod component;
pub mod project;
mod source;

pub use bundle::{bundle, script};
pub use component::compile;
pub use source::Templates;

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
    pub entry: Option<String>,
}
