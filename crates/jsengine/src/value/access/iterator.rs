use crate::value::{Atom, Call, Object};
use crate::{Access, Addons, Engine, Error, Failure, Invocation, Property, Value};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) enum State {
    Array(Value, u64),
    Text(String, usize),
    Done,
}

impl Engine {
    pub(crate) fn iterators(&mut self) {
        let values = self.function(Call::Values);
        self.signature(&values, "values", 0);
        self.objects[self.roots.array.slot]
            .fields
            .insert(Atom::Symbol(self.iterator), Property::hidden(values));
        let next = self.function(Call::Next);
        self.signature(&next, "next", 0);
        self.objects[self.roots.iterator.slot]
            .fields
            .insert("next".into(), Property::hidden(next));
        let identity = self.function(Call::Identity);
        self.signature(&identity, "[Symbol.iterator]", 0);
        self.objects[self.roots.iterator.slot]
            .fields
            .insert(Atom::Symbol(self.iterator), Property::hidden(identity));
        self.textiter = self.function(Call::Characters);
        let method = self.textiter.clone();
        self.signature(&method, "[Symbol.iterator]", 0);
    }

    fn iteratorobject(&mut self, state: State) -> Value {
        let mut object = Object::new(Default::default());
        object.prototype = Some(self.roots.iterator);
        object.iterator = Some(state);
        self.allocate(object)
    }

    pub(crate) fn values(&mut self, invocation: Invocation) -> Result<Value, Failure> {
        let value = invocation.receiver;
        if !matches!(value, Value::Object(_)) {
            return Err(self.raise(Error::Type, "array iterator requires an object"));
        }
        Ok(self.iteratorobject(State::Array(value, 0)))
    }

    pub(crate) fn textvalues(&mut self, invocation: Invocation) -> Result<Value, Failure> {
        let Value::Text(text) = invocation.receiver else {
            return Err(self.raise(Error::Type, "text iterator requires a text receiver"));
        };
        Ok(self.iteratorobject(State::Text(text, 0)))
    }

    pub(crate) fn nextvalue(
        &mut self,
        invocation: Invocation,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let Value::Object(handle) = invocation.receiver else {
            return Err(self.raise(Error::Type, "iterator next requires an iterator"));
        };
        let Some(state) = self.objects[handle.slot].iterator.clone() else {
            return Err(self.raise(Error::Type, "iterator next requires an iterator"));
        };
        let value = match state {
            State::Array(array, index) => {
                let length = self
                    .read(&Access::direct(&array, "length"), host)?
                    .number()?;
                let length = if length.is_nan() || length < 0.0 {
                    0
                } else {
                    length.min(u32::MAX as f64) as u64
                };
                if index >= length {
                    self.objects[handle.slot].iterator = Some(State::Done);
                    None
                } else {
                    self.objects[handle.slot].iterator =
                        Some(State::Array(array.clone(), index + 1));
                    Some(self.read(&Access::direct(&array, index.to_string()), host)?)
                }
            }
            State::Text(text, offset) => {
                if let Some(ch) = text[offset..].chars().next() {
                    self.objects[handle.slot].iterator =
                        Some(State::Text(text, offset + ch.len_utf8()));
                    Some(Value::Text(ch.to_string()))
                } else {
                    self.objects[handle.slot].iterator = Some(State::Done);
                    None
                }
            }
            State::Done => None,
        };
        Ok(self.object(BTreeMap::from([
            ("done".into(), Value::Boolean(value.is_none())),
            ("value".into(), value.unwrap_or(Value::Undefined)),
        ]))?)
    }
}
