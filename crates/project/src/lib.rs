mod adapter;
mod files;
mod frontend;
mod graph;
mod linker;
mod memory;
mod model;
mod resolver;

pub use adapter::{Adapter, Javascript};
pub use files::Files;
pub use frontend::{Binding, Body, Dependency, Export, Syntax, parse};
pub use graph::{Cache, Edge, Graph, Identity, Node, Output, Target};
pub use linker::Linker;
pub use memory::Memory;
pub use model::{
    Artifact, Diagnostic, Fault, Import, Kind, Module, Origin, Package, Request, Source, Span,
};
pub use resolver::Resolver;

pub trait Provider {
    fn read(&self, request: Request) -> Result<Source, Diagnostic>;
}

pub trait Resolve {
    fn resolve(&self, request: Import) -> Result<Module, Diagnostic>;
}
