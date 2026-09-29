use crate::contract::Fault;

pub use ::platform::shared::{Console, Display, Input};

pub trait Backend: ::platform::shared::Backend<Fault> {}
impl<T: ::platform::shared::Backend<Fault>> Backend for T {}
