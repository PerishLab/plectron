use std::sync::atomic::{AtomicU64, Ordering};

mod eval;
mod fault;
pub mod jit;
mod native;
mod parse;
mod scan;
mod scope;
mod syntax;
mod value;

pub use fault::{Failure, Fault, Rejection, Site, Source, Syntax, Weak};
pub use native::Invocation;
pub use scope::retention::{Collection, Reachability, Root};
use scope::{Closure, Scope};
use syntax::Function;
pub use value::error::Kind as Error;
pub use value::promise::{Deferred, Jobs};
pub use value::{Access, Atom, Content, Handle, Property, Symbol, Value};
use value::{Call, Construct, Roots};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);
const DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Activity {
    pub ticks: u64,
    pub calls: u64,
    pub closures: u64,
    pub natives: u64,
    pub reads: u64,
    pub writes: u64,
    pub lookups: u64,
    pub objects: u64,
    pub scopes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slots {
    pub objects: usize,
    pub scopes: usize,
    pub bindings: usize,
    pub closures: usize,
    pub resolvers: usize,
}

impl Activity {
    pub fn since(self, previous: Self) -> Self {
        Self {
            ticks: self.ticks.saturating_sub(previous.ticks),
            calls: self.calls.saturating_sub(previous.calls),
            closures: self.closures.saturating_sub(previous.closures),
            natives: self.natives.saturating_sub(previous.natives),
            reads: self.reads.saturating_sub(previous.reads),
            writes: self.writes.saturating_sub(previous.writes),
            lookups: self.lookups.saturating_sub(previous.lookups),
            objects: self.objects.saturating_sub(previous.objects),
            scopes: self.scopes.saturating_sub(previous.scopes),
        }
    }
}

pub trait Addons {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: Invocation,
    ) -> Result<Value, Failure>;

    fn roots(&self, _: &mut dyn FnMut(&Value)) {}

    fn edges(&self, _: Handle, _: &mut dyn FnMut(&Value)) {}

    fn native(&self, _: usize, _: &mut dyn FnMut(&Value)) {}

    fn sweep(&mut self, _: &[Handle], _: &dyn Fn(usize) -> bool) {}
}

pub struct Engine {
    site: Option<Site>,
    promises: value::promise::State,
    weak: scope::retention::weak::Registry,
    jit: jit::State,
    activity: Activity,
    collection: Collection,
    identity: u64,
    roots: Roots,
    retained: scope::retention::Registry,
    arenas: scope::Arenas,
    scopes: Vec<Scope>,
    bindings: Vec<scope::Binding>,
    objects: Vec<value::Object>,
    epochs: Vec<u64>,
    live: Vec<bool>,
    vacant: Vec<usize>,
    closures: Vec<Closure>,
    budget: usize,
    depth: usize,
    turns: usize,
    errors: std::collections::BTreeMap<Error, Value>,
    strings: std::collections::BTreeMap<&'static str, Value>,
    symbols: Vec<Option<String>>,
    iterator: Symbol,
    plectron: Symbol,
    textiter: Value,
}

impl Default for Engine {
    fn default() -> Self {
        let identity = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .expect("engine identity space exhausted");
        let roots = Roots::new(identity);
        let objects = roots.objects();
        let symbol = Symbol {
            owner: identity,
            slot: 0,
        };
        let mut engine = Self {
            site: None,
            promises: Default::default(),
            weak: Default::default(),
            jit: jit::State::default(),
            activity: Default::default(),
            collection: Default::default(),
            identity,
            roots,
            retained: scope::retention::Registry::default(),
            arenas: scope::Arenas::default(),
            scopes: vec![Scope::default()],
            bindings: Vec::new(),
            objects,
            epochs: vec![0; 4],
            live: vec![true; 4],
            vacant: Vec::new(),
            closures: Vec::new(),
            budget: 0,
            depth: 0,
            turns: 0,
            errors: std::collections::BTreeMap::new(),
            strings: std::collections::BTreeMap::new(),
            symbols: Vec::new(),
            iterator: symbol,
            plectron: symbol,
            textiter: Value::Undefined,
        };
        engine.arenas.scopes.allocate();
        engine
            .define(0, "undefined".into(), Value::Undefined, false)
            .expect("unique builtin");
        engine.builtins().expect("valid error intrinsics");
        engine.strings();
        engine.symbols().expect("valid Symbol intrinsic");
        engine.iterators();
        engine.arrays().expect("array intrinsic");
        engine.reflects().expect("valid Reflect intrinsic");
        engine.proxies().expect("valid Proxy intrinsic");
        engine.functions();
        engine.promises().expect("promise intrinsics");
        engine.weaks().expect("weak collection intrinsics");
        engine
    }
}

impl Engine {
    pub fn call(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        addons: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.invoke(callee, Invocation::direct(arguments), addons)
    }

    pub fn invoke(
        &mut self,
        callee: Value,
        invocation: Invocation,
        addons: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.enter();
        let result = observe::call("js.invoke", || self.apply(callee, invocation, addons));
        self.turns -= 1;
        result
    }

    pub fn construct(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        addons: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.enter();
        let result = observe::call("js.construct", || self.build(callee, arguments, addons));
        self.turns -= 1;
        result
    }

    fn enter(&mut self) {
        if self.turns == 0 {
            self.budget = 100_000;
        }
        self.turns += 1;
    }

    fn tick(&mut self) -> Result<(), Fault> {
        self.activity.ticks = self.activity.ticks.saturating_add(1);
        self.budget = self
            .budget
            .checked_sub(1)
            .ok_or_else(|| Fault("execution budget exhausted".into()))?;
        Ok(())
    }

    pub fn activity(&self) -> Activity {
        self.activity
    }

    pub fn collection(&self) -> Collection {
        self.collection
    }

    pub fn slots(&self) -> Slots {
        Slots {
            objects: self.objects.len(),
            scopes: self.scopes.len(),
            bindings: self.bindings.len(),
            closures: self.closures.len(),
            resolvers: self.promises.slots(),
        }
    }

    fn capture(&mut self, function: Function, scope: usize) -> usize {
        let slot = self.arenas.closures.allocate();
        let closure = Closure {
            code: function,
            scope,
            home: None,
        };
        if slot == self.closures.len() {
            self.closures.push(closure);
        } else {
            self.closures[slot] = closure;
        }
        slot
    }

    fn closure(&mut self, function: Function, scope: usize) -> Value {
        let length = function.params.arity();
        let slot = self.capture(function, scope);
        if self.closures[slot].code.asynchronous {
            let value = self.function(Call::Closure(slot));
            self.signature(&value, "", length);
            return value;
        }
        let prototype = self.ordinary(Default::default());
        let value = self.constructor(Call::Closure(slot), Construct::Closure(slot), prototype);
        self.signature(&value, "", length);
        value
    }

    fn arrow(&mut self, function: Function, scope: usize) -> Value {
        let length = function.params.arity();
        let slot = self.capture(function, scope);
        let value = self.function(Call::Arrow(slot));
        self.signature(&value, "", length);
        value
    }
}
