use crate::value::{Call, Construct};
use crate::{Addons, Engine, Error, Failure, Fault, Handle, Invocation, Property, Value};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Map,
    Set,
}

#[derive(Clone, Copy)]
pub(crate) enum Method {
    Constructor(Kind),
    Get,
    Set,
    Add,
    Has(Kind),
    Delete(Kind),
}

struct Data {
    kind: Kind,
    entries: HashMap<Handle, Value>,
}

#[derive(Default)]
pub(crate) struct Registry {
    entries: HashMap<Handle, Data>,
}

struct Pending {
    remaining: u8,
    value: Value,
}

pub(crate) struct Activation {
    pending: Vec<Pending>,
    reverse: HashMap<Handle, Vec<usize>>,
    known: HashSet<Handle>,
    activated: usize,
}

impl Registry {
    pub(crate) fn shape(&self) -> (usize, usize) {
        (
            self.entries.len(),
            self.entries.values().map(|data| data.entries.len()).sum(),
        )
    }

    pub(crate) fn activation(&self) -> Activation {
        let mut pending = Vec::new();
        let mut reverse: HashMap<Handle, Vec<usize>> = HashMap::new();
        for (owner, data) in &self.entries {
            if data.kind != Kind::Map {
                continue;
            }
            for (key, value) in &data.entries {
                let index = pending.len();
                pending.push(Pending {
                    remaining: u8::from(owner != key) + 1,
                    value: value.clone(),
                });
                reverse.entry(*owner).or_default().push(index);
                if owner != key {
                    reverse.entry(*key).or_default().push(index);
                }
            }
        }
        Activation {
            pending,
            reverse,
            known: HashSet::new(),
            activated: 0,
        }
    }

    pub(crate) fn sweep(&mut self, live: &HashSet<Handle>) {
        self.entries.retain(|owner, data| {
            if !live.contains(owner) {
                return false;
            }
            data.entries.retain(|key, _| live.contains(key));
            true
        });
    }
}

impl Activation {
    pub(crate) fn advance(&mut self, marked: &[Handle], values: &mut Vec<Value>) -> bool {
        let before = self.activated;
        for handle in marked {
            if !self.known.insert(*handle) {
                continue;
            }
            for index in self.reverse.get(handle).into_iter().flatten() {
                let edge = &mut self.pending[*index];
                edge.remaining -= 1;
                if edge.remaining == 0 {
                    values.push(edge.value.clone());
                    self.activated += 1;
                }
            }
        }
        self.activated != before
    }

    pub(crate) fn activated(&self) -> usize {
        self.activated
    }
}

impl Engine {
    pub(crate) fn weaks(&mut self) -> Result<(), Fault> {
        let maps = [
            ("get", Method::Get, 1),
            ("set", Method::Set, 2),
            ("has", Method::Has(Kind::Map), 1),
            ("delete", Method::Delete(Kind::Map), 1),
        ];
        let sets = [
            ("add", Method::Add, 1),
            ("has", Method::Has(Kind::Set), 1),
            ("delete", Method::Delete(Kind::Set), 1),
        ];
        for (name, kind, methods) in [
            ("WeakMap", Kind::Map, maps.as_slice()),
            ("WeakSet", Kind::Set, sets.as_slice()),
        ] {
            let prototype = self.object(Default::default())?;
            let constructor = self.constructor(
                Call::Weak(Method::Constructor(kind)),
                Construct::Weak(kind),
                prototype.clone(),
            );
            self.signature(&constructor, name, 0);
            for &(method, call, length) in methods {
                let function = self.function(Call::Weak(call));
                self.signature(&function, method, length);
                self.property(&prototype, method, Property::hidden(function))
                    .map_err(|failure| self.diagnostic(&failure))?;
            }
            self.register(name, constructor)?;
        }
        Ok(())
    }

    pub(crate) fn weakbuild(
        &mut self,
        kind: Kind,
        target: Value,
        arguments: Vec<Value>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        if !matches!(
            arguments.first(),
            None | Some(Value::Undefined | Value::Null)
        ) {
            return Err(self.raise(Error::Type, "weak collection iterable is unsupported"));
        }
        let value = self.creating(&target, host)?;
        let Value::Object(handle) = value else {
            unreachable!();
        };
        self.weak.entries.insert(
            handle,
            Data {
                kind,
                entries: HashMap::new(),
            },
        );
        Ok(value)
    }

    pub(crate) fn weakcall(
        &mut self,
        method: Method,
        invocation: Invocation,
    ) -> Result<Value, Failure> {
        if let Method::Constructor(_) = method {
            return Err(self.raise(Error::Type, "weak collection requires new"));
        }
        let kind = match method {
            Method::Get | Method::Set => Kind::Map,
            Method::Add => Kind::Set,
            Method::Has(kind) | Method::Delete(kind) | Method::Constructor(kind) => kind,
        };
        let Value::Object(owner) = invocation.receiver else {
            return Err(self.raise(Error::Type, "invalid weak collection receiver"));
        };
        if self
            .weak
            .entries
            .get(&owner)
            .is_none_or(|data| data.kind != kind)
        {
            return Err(self.raise(Error::Type, "invalid weak collection receiver"));
        }
        let key = invocation
            .arguments
            .first()
            .cloned()
            .unwrap_or(Value::Undefined);
        let Value::Object(key) = key else {
            return match method {
                Method::Get => Ok(Value::Undefined),
                Method::Has(_) | Method::Delete(_) => Ok(Value::Boolean(false)),
                _ => Err(self.raise(Error::Type, "weak collection key must be an object")),
            };
        };
        let data = self
            .weak
            .entries
            .get_mut(&owner)
            .expect("validated receiver");
        match method {
            Method::Get => Ok(data.entries.get(&key).cloned().unwrap_or(Value::Undefined)),
            Method::Has(_) => Ok(Value::Boolean(data.entries.contains_key(&key))),
            Method::Delete(_) => Ok(Value::Boolean(data.entries.remove(&key).is_some())),
            Method::Set => {
                let value = invocation
                    .arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Value::Undefined);
                data.entries.insert(key, value);
                Ok(Value::Object(owner))
            }
            Method::Add => {
                data.entries.insert(key, Value::Undefined);
                Ok(Value::Object(owner))
            }
            Method::Constructor(_) => unreachable!(),
        }
    }
}
