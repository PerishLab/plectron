mod access;
mod descriptor;
mod integrity;
mod invocation;
mod keys;

use crate::{
    Engine, Error, Failure, Fault, Property, Value,
    value::{Call, Construct, Object},
};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(crate) struct Proxy {
    pub target: Value,
    pub handler: Option<Value>,
}

impl Engine {
    pub(crate) fn proxies(&mut self) -> Result<(), Fault> {
        let constructor = self.function(Call::Factory);
        let Value::Object(handle) = constructor else {
            unreachable!();
        };
        self.objects[handle.slot].construct = Some(Construct::Factory);
        let constructor = Value::Object(handle);
        self.signature(&constructor, "Proxy", 2);
        let revocable = self.function(Call::Revocable);
        self.signature(&revocable, "revocable", 2);
        self.objects[handle.slot]
            .fields
            .insert("revocable".into(), Property::hidden(revocable));
        self.register("Proxy", constructor)
    }

    pub(crate) fn revocable(&mut self, args: &[Value]) -> Result<Value, Failure> {
        let target = args.first().cloned().unwrap_or(Value::Undefined);
        let handler = args.get(1).cloned().unwrap_or(Value::Undefined);
        let proxy = self.proxy(&target, &handler)?;
        let Value::Object(handle) = proxy else {
            unreachable!();
        };
        let revoke = self.function(Call::Revoke(handle));
        Ok(self.object(BTreeMap::from([
            ("proxy".into(), Value::Object(handle)),
            ("revoke".into(), revoke),
        ]))?)
    }

    pub fn proxy(&mut self, target: &Value, handler: &Value) -> Result<Value, Failure> {
        self.validate(target)?;
        self.validate(handler)?;
        if !matches!(target, Value::Object(_)) || !matches!(handler, Value::Object(_)) {
            return Err(self.raise(Error::Type, "Proxy target and handler must be objects"));
        }
        let mut object = Object::new(Default::default());
        if let Value::Object(handle) = target {
            if self.objects[handle.slot].call.is_some() {
                object.call = Some(Call::Proxy);
            }
            if self.objects[handle.slot].construct.is_some() {
                object.construct = Some(Construct::Proxy);
            }
        }
        object.proxy = Some(Proxy {
            target: target.clone(),
            handler: Some(handler.clone()),
        });
        Ok(self.allocate(object))
    }

    pub fn revoke(&mut self, value: &Value) -> Result<(), Fault> {
        self.validate(value)?;
        let Value::Object(handle) = value else {
            return Err(Fault("expected Proxy".into()));
        };
        let Some(proxy) = &mut self.objects[handle.slot].proxy else {
            return Err(Fault("expected Proxy".into()));
        };
        proxy.handler = None;
        Ok(())
    }

    pub(crate) fn proxydata(&self, value: &Value) -> Option<Proxy> {
        let Value::Object(handle) = value else {
            return None;
        };
        if self.validate(&Value::Object(*handle)).is_err() {
            return None;
        }
        self.objects[handle.slot].proxy.clone()
    }

    pub(crate) fn guard(&self, value: &Value) -> Result<(), Fault> {
        self.validate(value)?;
        if self.proxydata(value).is_some() {
            return Err(Fault("Proxy requires an execution context".into()));
        }
        Ok(())
    }
}
