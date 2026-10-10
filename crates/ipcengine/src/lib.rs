#![deny(unsafe_code)]

pub mod buffer;
pub mod endpoint;
mod fault;
pub mod file;
pub mod message;
#[cfg(unix)]
pub mod native;
pub mod observe;
pub mod stream;
pub mod transport;
pub use fault::Fault;
