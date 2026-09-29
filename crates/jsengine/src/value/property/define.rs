use super::{Content, Property};
use crate::{Engine, Error, Failure, Value, value::Atom};

impl Property {
    fn identical(&self, next: &Self) -> bool {
        if self.enumerable != next.enumerable || self.configurable != next.configurable {
            return false;
        }
        match (&self.content, &next.content) {
            (
                Content::Data {
                    value: before,
                    writable: old,
                },
                Content::Data {
                    value: after,
                    writable: new,
                },
            ) => old == new && same(before, after),
            (
                Content::Accessor {
                    get: oldget,
                    set: oldset,
                },
                Content::Accessor {
                    get: newget,
                    set: newset,
                },
            ) => oldget == newget && oldset == newset,
            _ => false,
        }
    }

    pub(crate) fn compatible(&self, next: &Self) -> bool {
        if self.configurable {
            return true;
        }
        if next.configurable || self.enumerable != next.enumerable {
            return false;
        }
        match (&self.content, &next.content) {
            (
                Content::Data {
                    value: before,
                    writable: false,
                },
                Content::Data {
                    value: after,
                    writable,
                },
            ) => !writable && same(before, after),
            (Content::Data { .. }, Content::Data { .. }) => true,
            (
                Content::Accessor {
                    get: oldget,
                    set: oldset,
                },
                Content::Accessor {
                    get: newget,
                    set: newset,
                },
            ) => oldget == newget && oldset == newset,
            _ => false,
        }
    }
}

pub(super) fn same(before: &Value, after: &Value) -> bool {
    match (before, after) {
        (Value::Number(left), Value::Number(right)) => {
            left.to_bits() == right.to_bits() || left.is_nan() && right.is_nan()
        }
        _ => before == after,
    }
}

impl Engine {
    pub fn configure(
        &mut self,
        object: &Value,
        key: &Atom,
        property: Property,
    ) -> Result<bool, Failure> {
        self.validate(object)?;
        if let Atom::Symbol(symbol) = key {
            self.validate(&Value::Symbol(*symbol))?;
        }
        match &property.content {
            Content::Data { value, .. } => self.validate(value)?,
            Content::Accessor { get, set } => {
                for callback in get.iter().chain(set) {
                    self.callable(callback)?;
                }
            }
        }
        let Value::Object(handle) = object else {
            return Err(self.raise(Error::Type, "expected object"));
        };
        let current = self.inspect(object, key)?;
        let target = &mut self.objects[handle.slot];
        if current.as_ref().is_some_and(|old| old.identical(&property)) {
            return Ok(true);
        }
        if target.frozen
            || current
                .as_ref()
                .is_some_and(|old| !old.compatible(&property))
        {
            return Ok(false);
        }
        if current.is_none() && !target.extensible {
            return Ok(false);
        }
        if target.length.is_some() && key.string() == Some("length") {
            let Content::Data {
                value,
                writable: true,
            } = &property.content
            else {
                return Ok(false);
            };
            let number = value.number()?;
            if !number.is_finite() || number != f64::from(number as u32) {
                return Err(self.raise(Error::Range, "invalid array length"));
            }
            target.set(key, value.clone())?;
            return Ok(true);
        }
        target.store(key, property)?;
        Ok(true)
    }
}
