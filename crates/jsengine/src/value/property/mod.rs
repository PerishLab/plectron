mod define;
mod descriptor;
mod proxy;
mod reflect;
pub(crate) use descriptor::Descriptor;
pub(crate) use proxy::Proxy;
pub(crate) use reflect::Method;

use crate::{Engine, Error, Failure, Fault, Value, value::Atom};

#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Data {
        value: Value,
        writable: bool,
    },
    Accessor {
        get: Option<Value>,
        set: Option<Value>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub content: Content,
    pub enumerable: bool,
    pub configurable: bool,
}

impl Property {
    pub(crate) fn data(value: Value) -> Self {
        Self {
            content: Content::Data {
                value,
                writable: true,
            },
            enumerable: true,
            configurable: true,
        }
    }

    pub(crate) fn hidden(value: Value) -> Self {
        Self {
            enumerable: false,
            ..Self::data(value)
        }
    }

    pub(crate) fn constant(value: Value) -> Self {
        Self {
            content: Content::Data {
                value,
                writable: false,
            },
            enumerable: false,
            configurable: true,
        }
    }

    pub fn fixed(value: Value, writable: bool) -> Self {
        Self {
            content: Content::Data { value, writable },
            enumerable: false,
            configurable: false,
        }
    }

    fn freeze(&mut self) {
        if let Content::Data { writable, .. } = &mut self.content {
            *writable = false;
        }
        self.configurable = false;
    }

    pub fn value(&self) -> Result<Value, Fault> {
        match &self.content {
            Content::Data { value, .. } => Ok(value.clone()),
            Content::Accessor { .. } => Err(Fault("accessor requires an execution context".into())),
        }
    }

    pub fn writable(&self) -> Option<bool> {
        match self.content {
            Content::Data { writable, .. } => Some(writable),
            Content::Accessor { .. } => None,
        }
    }
}

impl Engine {
    pub fn has(&self, value: &Value, name: &Atom) -> Result<bool, Fault> {
        self.guard(value)?;
        if let Atom::Symbol(symbol) = name {
            self.validate(&Value::Symbol(*symbol))?;
        }
        Ok(self.content(value, name)?.is_some())
    }

    pub fn delete(&mut self, value: &Value, name: &Atom) -> Result<bool, Failure> {
        let Some(property) = self.inspect(value, name)? else {
            return Ok(true);
        };
        if !property.configurable {
            return Ok(false);
        }
        let Value::Object(handle) = value else {
            unreachable!();
        };
        self.objects[handle.slot].remove(name);
        Ok(true)
    }

    pub fn extensible(&self, value: &Value) -> Result<bool, Fault> {
        self.guard(value)?;
        let Value::Object(handle) = value else {
            return Err(Fault("expected object".into()));
        };
        Ok(self.objects[handle.slot].extensible)
    }

    pub fn prevent(&mut self, value: &Value) -> Result<bool, Fault> {
        self.guard(value)?;
        let Value::Object(handle) = value else {
            return Err(Fault("expected object".into()));
        };
        self.objects[handle.slot].extensible = false;
        Ok(true)
    }

    pub fn descriptor(&self, value: &Value, name: &str) -> Result<Option<Property>, Fault> {
        self.inspect(value, &name.into())
    }

    pub(crate) fn inspect(&self, value: &Value, name: &Atom) -> Result<Option<Property>, Fault> {
        self.guard(value)?;
        if let Atom::Symbol(symbol) = name {
            self.validate(&Value::Symbol(*symbol))?;
        }
        let Value::Object(handle) = value else {
            return Err(Fault("expected object".into()));
        };
        let object = &self.objects[handle.slot];
        let mut property = object.lookup(name);
        if object.frozen
            && let Some(property) = &mut property
        {
            property.freeze();
        }
        Ok(property)
    }

    pub fn descriptors(
        &self,
        value: &Value,
    ) -> Result<std::collections::BTreeMap<String, Property>, Fault> {
        self.guard(value)?;
        let Value::Object(handle) = value else {
            return Err(Fault("expected object".into()));
        };
        let object = &self.objects[handle.slot];
        let mut properties = object.properties();
        if let Some(length) = object.own("length") {
            properties.insert("length".into(), length);
        }
        if object.frozen {
            for property in properties.values_mut() {
                property.freeze();
            }
        }
        Ok(properties)
    }

    pub(crate) fn inherited(&self, value: &Value, name: &str) -> Result<Option<Content>, Fault> {
        self.content(value, &name.into())
    }

    pub(crate) fn content(&self, value: &Value, name: &Atom) -> Result<Option<Content>, Fault> {
        let Value::Object(handle) = value else {
            return Err(Fault("expected object".into()));
        };
        let mut handle = *handle;
        loop {
            if self.objects[handle.slot].proxy.is_some() {
                return Err(Fault("Proxy requires an execution context".into()));
            }
            let object = &self.objects[handle.slot];
            if let Some(mut content) = object.content(name) {
                if object.frozen
                    && let Content::Data { writable, .. } = &mut content
                {
                    *writable = false;
                }
                return Ok(Some(content));
            }
            let Some(parent) = object.prototype else {
                return Ok(None);
            };
            handle = parent;
        }
    }

    pub(crate) fn property(
        &mut self,
        value: &Value,
        name: &str,
        property: Property,
    ) -> Result<(), Failure> {
        self.install(value, &name.into(), property)
    }

    pub(crate) fn install(
        &mut self,
        value: &Value,
        name: &Atom,
        property: Property,
    ) -> Result<(), Failure> {
        if self.configure(value, name, property)? {
            Ok(())
        } else {
            Err(self.raise(Error::Type, "property definition failed"))
        }
    }

    pub(crate) fn signature(&mut self, value: &Value, name: &str, length: usize) {
        let Value::Object(handle) = value else {
            unreachable!()
        };
        let fields = &mut self.objects[handle.slot].fields;
        fields.insert("name".into(), Property::constant(Value::Text(name.into())));
        fields.insert(
            "length".into(),
            Property::constant(Value::Number(length as f64)),
        );
    }
}
