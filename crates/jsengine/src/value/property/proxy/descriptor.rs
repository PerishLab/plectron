use super::{Proxy, access::keyvalue};
use crate::{
    Addons, Content, Engine, Error, Failure, Invocation, Property, Value,
    value::{Atom, property::Descriptor},
};

impl Engine {
    pub(crate) fn proxyobserve(
        &mut self,
        proxy: &Proxy,
        key: &Atom,
        host: &mut impl Addons,
    ) -> Result<Option<Property>, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "getOwnPropertyDescriptor", host)? else {
            return self.observing(&proxy.target, key, host);
        };
        let result = self.apply(
            trap,
            Invocation {
                receiver: handler,
                arguments: vec![proxy.target.clone(), keyvalue(key)],
            },
            host,
        )?;
        if result != Value::Undefined && !matches!(result, Value::Object(_)) {
            return Err(self.raise(Error::Type, "Proxy descriptor trap must return an object"));
        }
        let target = self.observing(&proxy.target, key, host)?;
        if result == Value::Undefined {
            if let Some(property) = target
                && (!property.configurable || !self.testing(&proxy.target, host)?)
            {
                return Err(self.raise(Error::Type, "Proxy descriptor concealed target property"));
            }
            return Ok(None);
        }
        let extensible = self.testing(&proxy.target, host)?;
        let property = self.parsing(&result, host)?.complete(None);
        if target.is_none() && !extensible {
            return Err(self.raise(Error::Type, "Proxy descriptor added to fixed target"));
        }
        if let Some(old) = &target
            && !old.compatible(&property)
        {
            return Err(self.raise(Error::Type, "Proxy descriptor is incompatible"));
        }
        if !property.configurable {
            let Some(old) = target else {
                return Err(self.raise(Error::Type, "Proxy descriptor invented fixed property"));
            };
            if old.configurable
                || matches!(
                    (&property.content, &old.content),
                    (
                        Content::Data {
                            writable: false,
                            ..
                        },
                        Content::Data { writable: true, .. }
                    )
                )
            {
                return Err(self.raise(Error::Type, "Proxy descriptor violated fixed property"));
            }
        }
        Ok(Some(property))
    }

    pub(crate) fn proxydefine(
        &mut self,
        proxy: &Proxy,
        key: &Atom,
        descriptor: Descriptor,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "defineProperty", host)? else {
            return self.definingown(&proxy.target, key, descriptor, host);
        };
        let value = descriptor.clone().object(self)?;
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![proxy.target.clone(), keyvalue(key), value],
                },
                host,
            )?
            .truth();
        if !result {
            return Ok(false);
        }
        let target = self.observing(&proxy.target, key, host)?;
        let extensible = self.testing(&proxy.target, host)?;
        if target.is_none() && !extensible {
            return Err(self.raise(Error::Type, "Proxy define added to fixed target"));
        }
        if descriptor.configurable == Some(false)
            && target.as_ref().is_none_or(|old| old.configurable)
        {
            return Err(self.raise(Error::Type, "Proxy define invented fixed property"));
        }
        if let Some(old) = target {
            let next = descriptor.clone().complete(Some(old.clone()));
            let freezing = descriptor.writable == Some(false)
                && !old.configurable
                && old.writable() == Some(true);
            if !old.compatible(&next) || freezing {
                return Err(self.raise(Error::Type, "Proxy define violated fixed property"));
            }
        }
        Ok(true)
    }
}
