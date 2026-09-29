use super::Proxy;
use crate::{Addons, Engine, Error, Failure, Invocation, Value, value::Atom};
use std::collections::HashSet;

impl Engine {
    pub(crate) fn proxykeys(
        &mut self,
        proxy: &Proxy,
        host: &mut impl Addons,
    ) -> Result<Vec<Atom>, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "ownKeys", host)? else {
            return self.listing(&proxy.target, host);
        };
        let object = self.apply(
            trap,
            Invocation {
                receiver: handler,
                arguments: vec![proxy.target.clone()],
            },
            host,
        )?;
        let values = self.reflectargs(&object, host)?;
        let mut keys = Vec::with_capacity(values.len());
        let mut unique = HashSet::new();
        for value in values {
            let key = match value {
                Value::Text(text) => Atom::String(text),
                Value::Symbol(symbol) => Atom::Symbol(symbol),
                _ => return Err(self.raise(Error::Type, "Proxy ownKeys requires property keys")),
            };
            if !unique.insert(key.clone()) {
                return Err(self.raise(Error::Type, "Proxy ownKeys returned duplicate key"));
            }
            keys.push(key);
        }
        let extensible = self.testing(&proxy.target, host)?;
        let target = self.listing(&proxy.target, host)?;
        for key in &target {
            let property = self.observing(&proxy.target, key, host)?;
            if (!extensible || property.is_some_and(|item| !item.configurable))
                && !unique.contains(key)
            {
                return Err(self.raise(Error::Type, "Proxy ownKeys omitted target key"));
            }
        }
        if !extensible && target.len() != keys.len() {
            return Err(self.raise(Error::Type, "Proxy ownKeys added key to fixed target"));
        }
        Ok(keys)
    }
}
