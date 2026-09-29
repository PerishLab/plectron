use crate::value::{Call, Construct, Object};
use crate::{Engine, Fault, Handle, Property, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Invocation {
    pub receiver: Value,
    pub arguments: Vec<Value>,
}

impl Invocation {
    pub fn direct(arguments: Vec<Value>) -> Self {
        Self {
            receiver: Value::Undefined,
            arguments,
        }
    }
}

impl Engine {
    pub fn derive(&mut self, prototype: &Value) -> Result<Value, Fault> {
        self.validate(prototype)?;
        if !matches!(prototype, Value::Object(_) | Value::Null) {
            return Err(Fault("prototype must be an object or null".into()));
        }
        let value = self.ordinary(BTreeMap::new());
        self.inherit(&value, Some(prototype));
        Ok(value)
    }
    pub fn register(&mut self, name: &str, value: Value) -> Result<(), Fault> {
        self.validate(&value)?;
        self.define(0, name.into(), value, false)
    }

    pub fn native(&mut self, binding: usize) -> Value {
        self.function(Call::Native(binding))
    }

    pub(crate) fn function(&mut self, call: Call) -> Value {
        let mut object = Object::new(BTreeMap::new());
        object.call = Some(call);
        object.prototype = Some(self.roots.function);
        self.allocate(object)
    }

    pub(crate) fn constructor(
        &mut self,
        call: Call,
        construct: Construct,
        prototype: Value,
    ) -> Value {
        let function = self.function(call);
        let Value::Object(handle) = &function else {
            unreachable!()
        };
        let object = &mut self.objects[handle.slot];
        object.construct = Some(construct);
        object.fields.insert(
            "prototype".into(),
            Property::fixed(prototype.clone(), matches!(call, Call::Closure(_))),
        );
        let Value::Object(handle) = prototype else {
            unreachable!()
        };
        self.objects[handle.slot]
            .fields
            .insert("constructor".into(), Property::hidden(function.clone()));
        function
    }

    pub(crate) fn allocate(&mut self, object: Object) -> Value {
        self.activity.objects = self.activity.objects.saturating_add(1);
        let slot = if let Some(slot) = self.vacant.pop() {
            self.epochs[slot] = self.epochs[slot]
                .checked_add(1)
                .expect("object identity space exhausted");
            self.objects[slot] = object;
            self.live[slot] = true;
            slot
        } else {
            let slot = self.objects.len();
            self.objects.push(object);
            self.epochs.push(0);
            self.live.push(true);
            slot
        };
        Value::Object(Handle {
            owner: self.identity,
            slot,
            epoch: self.epochs[slot],
        })
    }

    pub(crate) fn ordinary(&mut self, fields: BTreeMap<String, Value>) -> Value {
        let mut object = Object::new(fields);
        object.prototype = Some(self.roots.object);
        self.allocate(object)
    }

    pub fn object(&mut self, fields: BTreeMap<String, Value>) -> Result<Value, Fault> {
        for value in fields.values() {
            self.validate(value)?;
        }
        Ok(self.ordinary(fields))
    }

    pub fn properties(&self, value: &Value) -> Result<BTreeMap<String, Value>, Fault> {
        self.guard(value)?;
        match value {
            Value::Object(handle) => self.objects[handle.slot].values(),
            _ => Err(Fault("expected object".into())),
        }
    }

    pub fn callable(&self, value: &Value) -> Result<(), Fault> {
        self.validate(value)?;
        match value {
            Value::Object(handle) if self.objects[handle.slot].call.is_some() => Ok(()),
            _ => Err(Fault("value is not callable".into())),
        }
    }

    pub(crate) fn validate(&self, value: &Value) -> Result<(), Fault> {
        if let Value::Symbol(symbol) = value {
            if symbol.owner != self.identity || symbol.slot >= self.symbols.len() {
                return Err(Fault("Symbol belongs to another engine".into()));
            }
            return Ok(());
        }
        let Value::Object(handle) = value else {
            return Ok(());
        };
        if handle.owner != self.identity {
            return Err(Fault("value belongs to another engine".into()));
        }
        if !self.live.get(handle.slot).copied().unwrap_or(false)
            || self.epochs[handle.slot] != handle.epoch
        {
            return Err(Fault("invalid engine handle".into()));
        }
        Ok(())
    }
}
