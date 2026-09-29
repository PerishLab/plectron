mod files;
mod memory;
mod model;
mod resolver;

pub use files::Files;
pub use memory::Memory;
pub use model::{
    Artifact, Diagnostic, Fault, Import, Kind, Module, Package, Request, Source, Span,
};
pub use resolver::Resolver;

pub trait Provider {
    fn read(&self, request: Request) -> Result<Source, Diagnostic>;
}

pub trait Resolve {
    fn resolve(&self, request: Import) -> Result<Module, Diagnostic>;
}
