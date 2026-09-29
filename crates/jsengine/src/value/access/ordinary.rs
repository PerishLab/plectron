use super::Atom;
use crate::{Addons, Content, Engine, Error, Failure, Fault, Value, value::property::Descriptor};

impl Engine {
    pub fn get(&self, object: &Value, name: &str) -> Result<Value, Fault> {
        self.guard(object)?;
        if let Value::Text(text) = object {
            return Ok(self.text(text, name));
        }
        self.validate(object)?;
        match self.inherited(object, name)? {
            Some(Content::Data { value, .. }) => Ok(value),
            Some(Content::Accessor { .. }) => {
                Err(Fault("accessor requires an execution context".into()))
            }
            None => Ok(Value::Undefined),
        }
    }

    pub fn set(&mut self, object: &Value, name: &str, value: Value) -> Result<(), Failure> {
        self.guard(object)?;
        self.validate(&value)?;
        if !matches!(object, Value::Object(_)) {
            return Err(self.raise(Error::Type, "cannot assign property of non-object"));
        }
        if let Some(content) = self.inherited(object, name)? {
            match content {
                Content::Data {
                    writable: false, ..
                } => return Err(self.raise(Error::Type, "property is read-only")),
                Content::Accessor { .. } => {
                    return Err(Fault("accessor requires an execution context".into()).into());
                }
                _ => (),
            }
        }
        self.store(object, name, value)
    }

    pub(crate) fn store(
        &mut self,
        receiver: &Value,
        name: &str,
        value: Value,
    ) -> Result<(), Failure> {
        if self.save(receiver, &name.into(), value)? {
            Ok(())
        } else {
            Err(self.raise(Error::Type, "property assignment failed"))
        }
    }

    pub(crate) fn save(
        &mut self,
        receiver: &Value,
        name: &Atom,
        value: Value,
    ) -> Result<bool, Failure> {
        if self.proxydata(receiver).is_some() {
            return Err(self.raise(Error::Type, "Proxy define operation is unavailable"));
        }
        let Value::Object(handle) = receiver else {
            return Ok(false);
        };
        if self
            .inspect(receiver, name)?
            .is_some_and(|property| property.writable() != Some(true))
        {
            return Ok(false);
        }
        if self.objects[handle.slot].frozen {
            return Ok(false);
        }
        if !self.objects[handle.slot].extensible && self.objects[handle.slot].lookup(name).is_none()
        {
            return Ok(false);
        }
        if self.objects[handle.slot].length.is_some() && name.string() == Some("length") {
            let number = value.number()?;
            if !number.is_finite() || number != f64::from(number as u32) {
                return Err(self.raise(Error::Range, "invalid array length"));
            }
        }
        self.objects[handle.slot].set(name, value)?;
        Ok(true)
    }

    pub(crate) fn saving(
        &mut self,
        receiver: &Value,
        name: &Atom,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        if self.proxydata(receiver).is_none() {
            return self.save(receiver, name, value);
        }
        let current = self.observing(receiver, name, host)?;
        let descriptor = match current {
            Some(property) if property.writable() == Some(true) => Descriptor::assigned(value),
            Some(_) => return Ok(false),
            None => Descriptor::created(value),
        };
        self.definingown(receiver, name, descriptor, host)
    }
}
