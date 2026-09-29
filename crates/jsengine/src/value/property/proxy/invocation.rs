use super::Proxy;
use crate::{Addons, Engine, Error, Failure, Invocation, Value};

impl Engine {
    pub(crate) fn proxyapply(
        &mut self,
        proxy: &Proxy,
        invocation: Invocation,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "apply", host)? else {
            return self.apply(proxy.target.clone(), invocation, host);
        };
        let arguments = self.array(invocation.arguments.into_iter().map(Some).collect())?;
        self.apply(
            trap,
            Invocation {
                receiver: handler,
                arguments: vec![proxy.target.clone(), invocation.receiver, arguments],
            },
            host,
        )
    }

    pub(crate) fn proxybuild(
        &mut self,
        proxy: &Proxy,
        target: Value,
        arguments: Vec<Value>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let Some((handler, trap)) = self.trap(proxy, "construct", host)? else {
            return self.buildtarget(proxy.target.clone(), target, arguments, host);
        };
        let values = self.array(arguments.into_iter().map(Some).collect())?;
        let result = self.apply(
            trap,
            Invocation {
                receiver: handler,
                arguments: vec![proxy.target.clone(), values, target],
            },
            host,
        )?;
        if !matches!(result, Value::Object(_)) {
            return Err(self.raise(Error::Type, "Proxy construct must return an object"));
        }
        Ok(result)
    }
}
