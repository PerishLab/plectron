mod context;
pub(crate) mod retention;
use super::syntax::Statement;
use super::{Engine, Value};
use crate::{Error, Failure, Fault};
pub(crate) use context::Frame;

#[derive(Default)]
pub(crate) struct Arena {
    live: Vec<bool>,
    vacant: Vec<usize>,
}

impl Arena {
    pub(crate) fn allocate(&mut self) -> usize {
        if let Some(slot) = self.vacant.pop() {
            self.live[slot] = true;
            slot
        } else {
            let slot = self.live.len();
            self.live.push(true);
            slot
        }
    }

    pub(crate) fn release(&mut self, slot: usize) {
        self.live[slot] = false;
        self.vacant.push(slot);
    }

    pub(crate) fn contains(&self, slot: usize) -> bool {
        self.live[slot]
    }
}

#[derive(Default)]
pub(crate) struct Arenas {
    pub scopes: Arena,
    pub bindings: Arena,
    pub closures: Arena,
}

pub(super) struct Binding {
    pub(crate) value: Option<Value>,
    mutable: bool,
}
#[derive(Clone, Copy)]
pub(crate) struct Address(pub(crate) usize);

#[derive(Default)]
pub(super) struct Scope {
    pub(crate) parent: Option<usize>,
    pub(crate) frame: Option<Frame>,
    pub(crate) bindings: Vec<(String, Address)>,
}
impl Scope {
    fn binding(&self, name: &str) -> Option<Address> {
        self.bindings
            .iter()
            .rev()
            .find(|(candidate, _)| candidate == name)
            .map(|(_, address)| *address)
    }
}

pub(super) struct Closure {
    pub code: crate::syntax::Function,
    pub scope: usize,
    pub home: Option<Value>,
}

impl Engine {
    pub fn binding(&self, name: &str) -> Result<Value, Fault> {
        let address = self.scopes[0]
            .binding(name)
            .ok_or_else(|| Fault(format!("unknown binding: {name}")))?;
        self.bindings[address.0]
            .value
            .clone()
            .ok_or_else(|| Fault(format!("binding is not initialized: {name}")))
    }

    pub(crate) fn admit(&mut self, body: &[Statement]) -> Result<(), Failure> {
        for statement in body {
            if let Statement::Declare { name, .. } = statement.plain()
                && self.scopes[0].binding(name).is_some()
            {
                return Err(self
                    .raise(Error::Syntax, format!("duplicate binding: {name}"))
                    .at(statement.site()));
            }
        }
        Ok(())
    }

    pub(super) fn scope(&mut self, parent: usize) -> usize {
        self.activity.scopes = self.activity.scopes.saturating_add(1);
        let id = self.arenas.scopes.allocate();
        let scope = Scope {
            parent: Some(parent),
            ..Scope::default()
        };
        if id == self.scopes.len() {
            self.scopes.push(scope);
        } else {
            self.scopes[id] = scope;
        }
        id
    }

    pub(super) fn define(
        &mut self,
        scope: usize,
        name: String,
        value: Value,
        mutable: bool,
    ) -> Result<(), Fault> {
        let bindings = &mut self.scopes[scope].bindings;
        if bindings.iter().any(|(candidate, _)| candidate == &name) {
            return Err(Fault(format!("duplicate binding: {name}")));
        }
        let address = Address(self.arenas.bindings.allocate());
        let binding = Binding {
            value: Some(value),
            mutable,
        };
        if address.0 == self.bindings.len() {
            self.bindings.push(binding);
        } else {
            self.bindings[address.0] = binding;
        }
        bindings.push((name, address));
        Ok(())
    }

    pub(super) fn lookup(&mut self, scope: usize, name: &str) -> Result<Value, Failure> {
        let address = self
            .locate(scope, name)
            .ok_or_else(|| self.raise(Error::Reference, format!("unknown binding: {name}")))?;
        self.load(address, name)
    }
    pub(crate) fn load(&mut self, address: Address, name: &str) -> Result<Value, Failure> {
        self.bindings[address.0].value.clone().ok_or_else(|| {
            self.raise(
                Error::Reference,
                format!("binding is not initialized: {name}"),
            )
        })
    }

    pub(super) fn assign(
        &mut self,
        address: Address,
        name: &str,
        value: Value,
    ) -> Result<Value, Failure> {
        let binding = &mut self.bindings[address.0];
        if binding.value.is_none() {
            return Err(self.raise(
                Error::Reference,
                format!("binding is not initialized: {name}"),
            ));
        }
        if !binding.mutable {
            return Err(self.raise(Error::Type, format!("cannot assign const binding: {name}")));
        }
        binding.value = Some(value.clone());
        Ok(value)
    }

    pub(super) fn reserve(&mut self, scope: usize, name: &str, mutable: bool) -> Result<(), Fault> {
        self.define(scope, name.into(), Value::Undefined, mutable)?;
        let address = self.scopes[scope].binding(name).expect("reserved binding");
        self.bindings[address.0].value = None;
        Ok(())
    }

    pub(super) fn initialize(&mut self, scope: usize, name: &str, value: Value) {
        let address = self.scopes[scope].binding(name).expect("defined binding");
        self.bindings[address.0].value = Some(value);
    }

    pub(crate) fn locate(&mut self, mut scope: usize, name: &str) -> Option<Address> {
        self.activity.lookups = self.activity.lookups.saturating_add(1);
        loop {
            let current = &self.scopes[scope];
            if let Some(address) = current.binding(name) {
                return Some(address);
            }
            scope = current.parent?;
        }
    }
}
