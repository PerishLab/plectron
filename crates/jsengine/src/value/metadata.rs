use crate::{Handle, Value, syntax::Expression, value::Atom};

#[derive(Clone, Copy)]
pub(crate) enum Call {
    Proxy,
    Factory,
    Revocable,
    Revoke(Handle),
    Invoke,
    Promise(super::promise::Method),
    Resolve(usize, bool),
    Empty,
    Class,
    Closure(usize),
    Arrow(usize),
    Native(usize),
    Symbol,
    Reflect(super::property::Method),
    Error,
    Text(super::text::Method),
    Values,
    Characters,
    Next,
    Identity,
    Array,
    Arrays,
    Weak(crate::scope::retention::Method),
}

#[derive(Clone, Copy)]
pub(crate) enum Construct {
    Proxy,
    Factory,
    Promise,
    Closure(usize),
    Derived(Option<usize>),
    Error,
    Array,
    Weak(crate::scope::retention::Kind),
}

#[derive(Clone)]
pub(crate) struct Initializer {
    pub name: Atom,
    pub expression: Option<Expression>,
    pub scope: usize,
    pub home: Value,
}
