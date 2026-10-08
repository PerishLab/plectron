mod adapter;
mod admission;
mod diagnostic;
mod erasure;
mod frontend;

pub use adapter::Typescript;
pub use erasure::{Lowered, Repair};
use project::{Diagnostic, Source};

pub fn check(source: &Source) -> Result<(), Diagnostic> {
    frontend::read(source, |_, _| Ok(()))
}

pub fn erase(source: &Source) -> Result<Lowered, Diagnostic> {
    frontend::read(source, |semantic, tokens| {
        erasure::build(source, semantic, tokens)
    })
}
