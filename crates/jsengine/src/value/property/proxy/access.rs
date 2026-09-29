use super::Proxy;
use crate::value::property::define::same;
use crate::{
    Access, Addons, Content, Engine, Error, Failure, Invocation, Value,
    value::{Atom, View},
};

pub(super) fn keyvalue(key: &Atom) -> Value {
    match key {
        Atom::String(value) => Value::Text(value.clone()),
        Atom::Symbol(value) => Value::Symbol(*value),
    }
}

impl Engine {
    pub(super) fn trap(
        &mut self,
        proxy: &Proxy,
        name: &str,
        host: &mut impl Addons,
    ) -> Result<Option<(Value, Value)>, Failure> {
        let Some(handler) = &proxy.handler else {
            return Err(self.raise(Error::Type, "Proxy has been revoked"));
        };
        let method = self.read(&Access::direct(handler, name), host)?;
        if matches!(method, Value::Undefined | Value::Null) {
            return Ok(None);
        }
        if self.callable(&method).is_err() {
            return Err(self.raise(Error::Type, "Proxy trap is not callable"));
        }
        Ok(Some((handler.clone(), method)))
    }

    pub(crate) fn proxyget(
        &mut self,
        proxy: &Proxy,
        key: &Atom,
        receiver: &Value,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "get", host)? else {
            return self.reading(
                View {
                    base: &proxy.target,
                    receiver,
                    key: key.clone(),
                },
                host,
            );
        };
        let result = self.apply(
            trap,
            Invocation {
                receiver: handler,
                arguments: vec![proxy.target.clone(), keyvalue(key), receiver.clone()],
            },
            host,
        )?;
        if let Some(property) = self.observing(&proxy.target, key, host)?
            && !property.configurable
        {
            match property.content {
                Content::Data {
                    value,
                    writable: false,
                } if !same(&value, &result) => {
                    return Err(self.raise(Error::Type, "Proxy get violated fixed data property"));
                }
                Content::Accessor { get: None, .. } if result != Value::Undefined => {
                    return Err(self.raise(Error::Type, "Proxy get violated getter absence"));
                }
                _ => (),
            }
        }
        Ok(result)
    }

    pub(crate) fn proxyset(
        &mut self,
        proxy: &Proxy,
        key: &Atom,
        pair: (Value, &Value),
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let (value, receiver) = pair;
        let Some((handler, trap)) = self.trap(proxy, "set", host)? else {
            return self.attempting(
                View {
                    base: &proxy.target,
                    receiver,
                    key: key.clone(),
                },
                value,
                host,
            );
        };
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![
                        proxy.target.clone(),
                        keyvalue(key),
                        value.clone(),
                        receiver.clone(),
                    ],
                },
                host,
            )?
            .truth();
        if result
            && let Some(property) = self.observing(&proxy.target, key, host)?
            && !property.configurable
        {
            match property.content {
                Content::Data {
                    value: old,
                    writable: false,
                } if !same(&old, &value) => {
                    return Err(self.raise(Error::Type, "Proxy set violated fixed data property"));
                }
                Content::Accessor { set: None, .. } => {
                    return Err(self.raise(Error::Type, "Proxy set violated setter absence"));
                }
                _ => (),
            }
        }
        Ok(result)
    }

    pub(crate) fn proxyhas(
        &mut self,
        proxy: &Proxy,
        key: &Atom,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "has", host)? else {
            return self.checking(&proxy.target, key, host);
        };
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![proxy.target.clone(), keyvalue(key)],
                },
                host,
            )?
            .truth();
        if !result
            && let Some(property) = self.observing(&proxy.target, key, host)?
            && (!property.configurable || !self.testing(&proxy.target, host)?)
        {
            return Err(self.raise(Error::Type, "Proxy has concealed target property"));
        }
        Ok(result)
    }

    pub(crate) fn proxydelete(
        &mut self,
        proxy: &Proxy,
        key: &Atom,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "deleteProperty", host)? else {
            return self.removing(&proxy.target, key, host);
        };
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![proxy.target.clone(), keyvalue(key)],
                },
                host,
            )?
            .truth();
        if result
            && let Some(property) = self.observing(&proxy.target, key, host)?
            && (!property.configurable || !self.testing(&proxy.target, host)?)
        {
            return Err(self.raise(Error::Type, "Proxy delete concealed target property"));
        }
        Ok(result)
    }
}
