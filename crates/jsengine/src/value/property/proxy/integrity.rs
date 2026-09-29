use super::Proxy;
use crate::{Addons, Engine, Error, Failure, Invocation, Value};

impl Engine {
    pub(crate) fn proxytest(
        &mut self,
        proxy: &Proxy,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "isExtensible", host)? else {
            return self.testing(&proxy.target, host);
        };
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![proxy.target.clone()],
                },
                host,
            )?
            .truth();
        if result != self.testing(&proxy.target, host)? {
            return Err(self.raise(Error::Type, "Proxy extensibility disagrees with target"));
        }
        Ok(result)
    }

    pub(crate) fn proxyprevent(
        &mut self,
        proxy: &Proxy,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "preventExtensions", host)? else {
            return self.preventing(&proxy.target, host);
        };
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![proxy.target.clone()],
                },
                host,
            )?
            .truth();
        if result && self.testing(&proxy.target, host)? {
            return Err(self.raise(Error::Type, "Proxy prevented an extensible target"));
        }
        Ok(result)
    }

    pub(crate) fn proxyparent(
        &mut self,
        proxy: &Proxy,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "getPrototypeOf", host)? else {
            return self.parenting(&proxy.target, host);
        };
        let result = self.apply(
            trap,
            Invocation {
                receiver: handler,
                arguments: vec![proxy.target.clone()],
            },
            host,
        )?;
        self.validate(&result)?;
        if !matches!(result, Value::Object(_) | Value::Null) {
            return Err(self.raise(Error::Type, "Proxy prototype must be an object or null"));
        }
        if !self.testing(&proxy.target, host)? && result != self.parenting(&proxy.target, host)? {
            return Err(self.raise(Error::Type, "Proxy prototype disagrees with fixed target"));
        }
        Ok(result)
    }

    pub(crate) fn proxyrebase(
        &mut self,
        proxy: &Proxy,
        parent: &Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.validate(parent)?;
        if !matches!(parent, Value::Object(_) | Value::Null) {
            return Err(self.raise(Error::Type, "prototype must be an object or null"));
        }
        let Some((handler, trap)) = self.trap(proxy, "setPrototypeOf", host)? else {
            return self.rebasing(&proxy.target, parent, host);
        };
        let result = self
            .apply(
                trap,
                Invocation {
                    receiver: handler,
                    arguments: vec![proxy.target.clone(), parent.clone()],
                },
                host,
            )?
            .truth();
        if result
            && !self.testing(&proxy.target, host)?
            && *parent != self.parenting(&proxy.target, host)?
        {
            return Err(self.raise(Error::Type, "Proxy rebase disagrees with fixed target"));
        }
        Ok(result)
    }
}
