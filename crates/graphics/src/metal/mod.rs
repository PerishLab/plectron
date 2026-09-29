#![cfg(target_os = "macos")]

mod device;
mod residency;
pub use residency::{Residency, Resource, pool};
mod buffer;
pub use buffer::Batches;
mod drawing;
mod output;
mod resource;
mod runtime;
mod surface;
mod vertices;

pub use device::{Metal, Setup, Work};
pub use runtime::{Execution, Executor};
pub use surface::Surface;

fn error(value: impl std::fmt::Display) -> crate::shared::Fault {
    crate::shared::Fault(value.to_string())
}

#[cfg(test)]
#[path = "../../tests/metal/unit/mod.rs"]
mod checks;
