#![deny(unsafe_code)]

mod context;
mod guard;
#[allow(unsafe_code)]
mod native;
mod record;
mod session;
mod transfer;
pub use context::{Context, current};
pub use guard::Guard;
pub use record::{Kind, Options, Outcome, Record, Report, Status, Vertex};
pub use session::Session;
pub use transfer::{Envelope, Transfer};

pub fn resume(context: Option<&Context>, name: impl Into<String>) -> Guard {
    match context {
        Some(context) => context.scope(name),
        None if current().is_some() => Guard::blocked(),
        None => Guard::empty(),
    }
}

pub fn scope(name: impl Into<String>) -> Guard {
    current().map_or_else(Guard::empty, |context| context.scope(name))
}

pub fn call<T, E>(name: impl Into<String>, execute: impl FnOnce() -> Result<T, E>) -> Result<T, E> {
    let mut scope = scope(name);
    let result = execute();
    scope.finish(result.is_ok());
    result
}

pub fn node<T, E>(
    identity: (usize, &'static str),
    execute: impl FnOnce() -> Result<T, E>,
) -> Result<T, E> {
    let mut scope = current().map_or_else(Guard::empty, |context| {
        context.open(identity.1.into(), Kind::Node { index: identity.0 })
    });
    let result = execute();
    scope.finish(result.is_ok());
    result
}

pub fn graph(shape: impl FnOnce() -> Vec<Vertex>) -> Guard {
    current().map_or_else(Guard::empty, |context| {
        context.open("pipeline".into(), Kind::Graph { vertices: shape() })
    })
}
