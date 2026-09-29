use super::task::{Step, Task};
use crate::eval::iteration::Cursor;
use crate::syntax::{Body, Item, Key};
use crate::{Addons, Engine, Failure, Fault, Invocation, Property, Value};
use std::collections::VecDeque;

pub(super) struct Collection {
    items: VecDeque<Option<Item>>,
    values: Vec<Option<Value>>,
    scope: usize,
    call: Option<(Value, Value, bool)>,
}
pub(super) struct Fields {
    pub object: Value,
    pub fields: VecDeque<(Key, Body)>,
    pub scope: usize,
}
impl Collection {
    pub(super) fn trace(&self, value: &mut dyn FnMut(&Value), scope: &mut dyn FnMut(usize)) {
        scope(self.scope);
        for item in self.values.iter().flatten() {
            value(item);
        }
        if let Some((callee, receiver, _)) = &self.call {
            value(callee);
            value(receiver);
        }
    }

    pub fn array(items: Vec<Option<Item>>, scope: usize) -> Self {
        Self {
            items: items.into(),
            values: Vec::new(),
            scope,
            call: None,
        }
    }
    pub fn call(items: Vec<Item>, scope: usize, call: (Value, Value, bool)) -> Self {
        Self {
            items: items.into_iter().map(Some).collect(),
            values: Vec::new(),
            scope,
            call: Some(call),
        }
    }
}
impl Collection {
    fn next(&mut self) -> Option<(crate::syntax::Expression, bool)> {
        while let Some(item) = self.items.pop_front() {
            match item {
                None => self.values.push(None),
                Some(Item::Value(value)) => return Some((value, false)),
                Some(Item::Spread(value)) => return Some((value, true)),
            }
        }
        None
    }
    fn finish(self, engine: &mut Engine, host: &mut impl Addons) -> Result<Value, Failure> {
        let Some((callee, receiver, construct)) = self.call else {
            return Ok(engine.array(self.values)?);
        };
        let arguments = self
            .values
            .into_iter()
            .map(|value| value.unwrap())
            .collect();
        if construct {
            engine.build(callee, arguments, host)
        } else {
            engine.apply(
                callee,
                Invocation {
                    receiver,
                    arguments,
                },
                host,
            )
        }
    }
    fn append(
        &mut self,
        input: (Value, bool),
        engine: &mut Engine,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let (value, spread) = input;
        if !spread {
            self.values.push(Some(value));
            return Ok(());
        }
        let mut cursor = Cursor::new(value, engine, host)?;
        while let Some(value) = cursor.next(engine, host)? {
            engine.tick()?;
            if self.values.len() >= 4096 {
                return Err(Fault("sequence budget exhausted".into()).into());
            }
            self.values.push(Some(value));
        }
        Ok(())
    }
}
impl Task {
    fn key(&mut self, key: Key, scope: usize) {
        match key {
            Key::Literal(name) => self.values.push(Value::Text(name)),
            Key::Computed(value) => self.expression(value, scope),
        }
    }
    pub(super) fn collect(
        &mut self,
        step: Step,
        engine: &mut Engine,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        match step {
            Step::Collect(mut collection) => {
                if let Some((value, spread)) = collection.next() {
                    let scope = collection.scope;
                    self.steps.push(Step::Append(collection, spread));
                    self.expression(value, scope);
                } else {
                    self.values.push(collection.finish(engine, host)?);
                }
            }
            Step::Append(mut collection, spread) => {
                let value = self.pop();
                collection.append((value, spread), engine, host)?;
                self.steps.push(Step::Collect(collection));
            }
            Step::Fields(mut fields) => {
                if let Some((key, body)) = fields.fields.pop_front() {
                    let scope = fields.scope;
                    self.steps.push(Step::Field(fields, body));
                    self.key(key, scope);
                } else {
                    self.values.push(fields.object);
                }
            }
            Step::Field(fields, body) => {
                let key = self.pop().key()?;
                if let Body::Field(Some(value)) = body {
                    let scope = fields.scope;
                    self.steps.push(Step::Store(fields, key.clone()));
                    self.steps
                        .push(Step::Named(value, scope, key.string().map(str::to_owned)));
                } else {
                    let mut property =
                        engine.method((&fields.object, &key), &body, fields.scope)?;
                    property.enumerable = true;
                    engine.install(&fields.object, &key, property)?;
                    self.steps.push(Step::Fields(fields));
                }
            }
            Step::Store(fields, key) => {
                let value = self.pop();
                engine.install(&fields.object, &key, Property::data(value))?;
                self.steps.push(Step::Fields(fields));
            }
            _ => unreachable!("continuation step dispatcher"),
        }
        Ok(())
    }
}
