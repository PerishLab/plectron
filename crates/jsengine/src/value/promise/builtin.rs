use super::Reaction;
use crate::value::{Call, Construct};
use crate::{Addons, Engine, Error, Failure, Fault, Invocation, Property, Value};

#[derive(Clone, Copy)]
pub(crate) enum Method {
    Constructor,
    Resolve,
    Reject,
    Then,
    Catch,
    Finally,
}

impl Engine {
    pub(crate) fn promises(&mut self) -> Result<(), Fault> {
        let prototype = self.object(Default::default())?;
        let constructor = self.constructor(
            Call::Promise(Method::Constructor),
            Construct::Promise,
            prototype.clone(),
        );
        self.signature(&constructor, "Promise", 1);
        for (name, method) in [("resolve", Method::Resolve), ("reject", Method::Reject)] {
            let function = self.function(Call::Promise(method));
            self.signature(&function, name, if name == "then" { 2 } else { 1 });
            self.property(&constructor, name, Property::hidden(function))
                .map_err(|failure| self.diagnostic(&failure))?;
        }
        for (name, method) in [
            ("then", Method::Then),
            ("catch", Method::Catch),
            ("finally", Method::Finally),
        ] {
            let function = self.function(Call::Promise(method));
            self.signature(&function, name, if name == "then" { 2 } else { 1 });
            self.property(&prototype, name, Property::hidden(function))
                .map_err(|failure| self.diagnostic(&failure))?;
        }
        self.promises.prototype = Some(prototype);
        self.register("Promise", constructor)
    }
    pub(crate) fn executor(
        &mut self,
        arguments: Vec<Value>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let executor = arguments.first().cloned().unwrap_or(Value::Undefined);
        if self.callable(&executor).is_err() {
            return Err(self.raise(Error::Type, "Promise executor must be callable"));
        }
        let deferred = self.deferred()?;
        match self.apply(
            executor,
            Invocation::direct(vec![deferred.resolve, deferred.reject.clone()]),
            host,
        ) {
            Err(failure) if failure.thrown().is_some() => {
                self.rejected(&deferred.reject, failure)?;
            }
            Err(failure) => return Err(failure),
            _ => (),
        }
        Ok(deferred.promise)
    }
    pub(crate) fn promising(
        &mut self,
        method: Method,
        invocation: Invocation,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let first = invocation
            .arguments
            .first()
            .cloned()
            .unwrap_or(Value::Undefined);
        match method {
            Method::Constructor => Err(self.raise(Error::Type, "Promise requires new")),
            Method::Resolve | Method::Reject => {
                if matches!(method, Method::Resolve) {
                    return self.resolved(first, host);
                }
                let promise = self.promise()?;
                self.fulfill(&promise, Err(Failure::Throw(first).at(self.site.clone())))?;
                Ok(promise)
            }
            Method::Then | Method::Catch | Method::Finally => {
                if self.slot(&invocation.receiver).is_err() {
                    return Err(
                        self.raise(Error::Type, "promise method requires a promise receiver")
                    );
                }
                let target = self.promise()?;
                let reaction = self.handlers(target.clone(), (method, &invocation.arguments));
                self.subscribe(&invocation.receiver, reaction)?;
                Ok(target)
            }
        }
    }
}

impl Engine {
    fn handlers(&self, target: Value, input: (Method, &[Value])) -> Reaction {
        let (method, arguments) = input;
        let callable = |value: Value| self.callable(&value).ok().map(|()| value);
        let first = arguments.first().cloned().and_then(callable);
        if matches!(method, Method::Finally) {
            return match first {
                Some(callback) => Reaction::Finally { target, callback },
                None => Reaction::Forward {
                    target,
                    preserved: None,
                },
            };
        }
        if matches!(method, Method::Catch) {
            return Reaction::Then {
                target,
                yes: None,
                no: first,
            };
        }
        Reaction::Then {
            target,
            yes: first,
            no: arguments.get(1).cloned().and_then(callable),
        }
    }
}
