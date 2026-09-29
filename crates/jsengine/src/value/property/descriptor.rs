use super::{Content, Property};
use crate::{Addons, Engine, Error, Failure, Value, value::Atom};
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(crate) struct Descriptor {
    pub value: Option<Value>,
    pub writable: Option<bool>,
    pub get: Option<Value>,
    pub set: Option<Value>,
    pub enumerable: Option<bool>,
    pub configurable: Option<bool>,
}

impl Descriptor {
    pub fn assigned(value: Value) -> Self {
        Self {
            value: Some(value),
            ..Self::default()
        }
    }

    pub fn created(value: Value) -> Self {
        Self {
            value: Some(value),
            writable: Some(true),
            enumerable: Some(true),
            configurable: Some(true),
            ..Self::default()
        }
    }

    pub fn from(property: Property) -> Self {
        let mut descriptor = Self {
            enumerable: Some(property.enumerable),
            configurable: Some(property.configurable),
            ..Self::default()
        };
        match property.content {
            Content::Data { value, writable } => {
                descriptor.value = Some(value);
                descriptor.writable = Some(writable);
            }
            Content::Accessor { get, set } => {
                descriptor.get = Some(get.unwrap_or(Value::Undefined));
                descriptor.set = Some(set.unwrap_or(Value::Undefined));
            }
        }
        descriptor
    }

    pub fn complete(self, current: Option<Property>) -> Property {
        let accessor = self.get.is_some() || self.set.is_some();
        let data = self.value.is_some() || self.writable.is_some();
        let (enumerable, configurable) = current
            .as_ref()
            .map(|property| (property.enumerable, property.configurable))
            .unwrap_or((false, false));
        let mut property = if accessor {
            current
                .filter(|property| matches!(property.content, Content::Accessor { .. }))
                .unwrap_or(Property {
                    content: Content::Accessor {
                        get: None,
                        set: None,
                    },
                    enumerable,
                    configurable,
                })
        } else if data {
            current
                .filter(|property| matches!(property.content, Content::Data { .. }))
                .unwrap_or(Property {
                    content: Content::Data {
                        value: Value::Undefined,
                        writable: false,
                    },
                    enumerable,
                    configurable,
                })
        } else {
            current.unwrap_or(Property {
                content: Content::Data {
                    value: Value::Undefined,
                    writable: false,
                },
                enumerable,
                configurable,
            })
        };
        if let Some(next) = self.enumerable {
            property.enumerable = next;
        }
        if let Some(next) = self.configurable {
            property.configurable = next;
        }
        match &mut property.content {
            Content::Data { value, writable } => {
                if let Some(next) = self.value {
                    *value = next;
                }
                if let Some(next) = self.writable {
                    *writable = next;
                }
            }
            Content::Accessor { get, set } => {
                if let Some(next) = self.get {
                    *get = (next != Value::Undefined).then_some(next);
                }
                if let Some(next) = self.set {
                    *set = (next != Value::Undefined).then_some(next);
                }
            }
        }
        property
    }

    pub fn object(self, engine: &mut Engine) -> Result<Value, Failure> {
        let mut fields = BTreeMap::new();
        for (name, value) in [
            ("value", self.value),
            ("get", self.get),
            ("set", self.set),
            ("writable", self.writable.map(Value::Boolean)),
            ("enumerable", self.enumerable.map(Value::Boolean)),
            ("configurable", self.configurable.map(Value::Boolean)),
        ] {
            if let Some(value) = value {
                fields.insert(name.into(), value);
            }
        }
        Ok(engine.object(fields)?)
    }
}

impl Engine {
    pub(crate) fn parsing(
        &mut self,
        object: &Value,
        host: &mut impl Addons,
    ) -> Result<Descriptor, Failure> {
        if !matches!(object, Value::Object(_)) {
            return Err(self.raise(Error::Type, "property descriptor must be an object"));
        }
        let descriptor = Descriptor {
            enumerable: self.field(object, "enumerable", host)?.map(|v| v.truth()),
            configurable: self.field(object, "configurable", host)?.map(|v| v.truth()),
            value: self.field(object, "value", host)?,
            writable: self.field(object, "writable", host)?.map(|v| v.truth()),
            get: self.field(object, "get", host)?,
            set: self.field(object, "set", host)?,
        };
        if (descriptor.get.is_some() || descriptor.set.is_some())
            && (descriptor.value.is_some() || descriptor.writable.is_some())
        {
            return Err(self.raise(Error::Type, "mixed property descriptor"));
        }
        for callback in descriptor.get.iter().chain(descriptor.set.iter()) {
            if *callback != Value::Undefined && self.callable(callback).is_err() {
                return Err(self.raise(Error::Type, "accessor must be callable"));
            }
        }
        Ok(descriptor)
    }

    fn field(
        &mut self,
        object: &Value,
        name: &str,
        host: &mut impl Addons,
    ) -> Result<Option<Value>, Failure> {
        let key = Atom::String(name.into());
        if !self.checking(object, &key, host)? {
            return Ok(None);
        }
        Ok(Some(self.reading(
            crate::value::View {
                base: object,
                receiver: object,
                key,
            },
            host,
        )?))
    }
}
