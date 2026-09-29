#[allow(unsafe_code)]
mod polling;
pub(crate) use polling::wait;
mod channel;
#[allow(unsafe_code)]
mod mapping;
mod publisher;
mod receiver;
#[allow(unsafe_code)]
mod rights;
pub use publisher::{Phase, Publisher};
pub use receiver::{Lease, Receiver};
