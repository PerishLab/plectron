#![cfg(windows)]

mod buffer;
pub use buffer::Batches;
mod device;
mod drawing;
mod ffi;
mod memory;
mod output;
mod residency;
pub use residency::{Residency, Resource, pool};
mod resource;
mod vertices;

pub use device::{D3d12, Setup, Work};

fn error(value: impl std::fmt::Display) -> crate::shared::Fault {
    crate::shared::Fault(value.to_string())
}

#[cfg(test)]
#[path = "../../tests/d3d12/unit/mod.rs"]
mod checks;
