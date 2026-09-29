use super::{Engine, Fault, Flow, Host, Value};
mod construction;
mod method;
mod parameters;
use crate::scope::Frame;
use crate::value::Call;
use crate::{Error, Failure, Invocation};

impl Engine {
    pub(crate) fn apply(
        &mut self,
        callee: Value,
        invocation: Invocation,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        self.tick()?;
        self.validate(&callee)?;
        self.validate(&invocation.receiver)?;
        for value in &invocation.arguments {
            self.validate(value)?;
        }
        self.activity.calls = self.activity.calls.saturating_add(1);
        let Value::Object(handle) = &callee else {
            return Err(self.raise(Error::Type, "value is not callable"));
        };
        match self.objects[handle.slot].call {
            Some(Call::Closure(_) | Call::Arrow(_)) => {
                self.activity.closures = self.activity.closures.saturating_add(1);
            }
            Some(Call::Native(_)) => {
                self.activity.natives = self.activity.natives.saturating_add(1);
            }
            _ => (),
        }
        if let Some(Call::Closure(slot) | Call::Arrow(slot)) = self.objects[handle.slot].call
            && let Some(result) = self.accelerate(slot, &invocation.arguments)
        {
            return result;
        }
        match self.objects[handle.slot].call {
            Some(Call::Proxy) => {
                let proxy = self.proxydata(&callee).unwrap();
                self.proxyapply(&proxy, invocation, host)
            }
            Some(Call::Factory) => Err(self.raise(Error::Type, "Proxy constructor requires new")),
            Some(Call::Revocable) => self.revocable(&invocation.arguments),
            Some(Call::Revoke(handle)) => {
                self.revoke(&Value::Object(handle))?;
                Ok(Value::Undefined)
            }
            Some(Call::Invoke) => {
                let mut arguments = invocation.arguments.into_iter();
                let receiver = arguments.next().unwrap_or(Value::Undefined);
                self.apply(
                    invocation.receiver,
                    Invocation {
                        receiver,
                        arguments: arguments.collect(),
                    },
                    host,
                )
            }
            Some(Call::Promise(method)) => self.promising(method, invocation, host),
            Some(Call::Resolve(id, reject)) => {
                self.settle(
                    id,
                    (
                        reject,
                        invocation
                            .arguments
                            .first()
                            .cloned()
                            .unwrap_or(Value::Undefined),
                    ),
                    host,
                )?;
                Ok(Value::Undefined)
            }
            Some(Call::Class) => Err(self.raise(Error::Type, "class constructor requires new")),
            Some(Call::Empty) => Ok(Value::Undefined),
            Some(Call::Closure(slot)) => self.invocation(slot, invocation, host),
            Some(Call::Arrow(slot)) => {
                let scope = self.scope(self.closures[slot].scope);
                self.body((slot, scope), invocation.arguments, host)
            }
            Some(Call::Native(binding)) => {
                let result = host.call(self, binding, invocation);
                if let Err(Failure::Throw(value)) = &result {
                    self.validate(value)?;
                }
                let value = result?;
                self.validate(&value)?;
                Ok(value)
            }
            Some(Call::Symbol) => self.symbol(&invocation.arguments),
            Some(Call::Reflect(method)) => self.reflect(method, invocation.arguments, host),
            Some(Call::Error) => self.build(callee, invocation.arguments, host),
            Some(Call::Text(method)) => self.string(method, invocation),
            Some(Call::Values) => self.values(invocation),
            Some(Call::Characters) => self.textvalues(invocation),
            Some(Call::Next) => self.nextvalue(invocation, host),
            Some(Call::Identity) => Ok(invocation.receiver),
            Some(Call::Array) => self.arraycall(&invocation.arguments),
            Some(Call::Arrays) => self.isarray(&invocation.arguments),
            Some(Call::Weak(method)) => self.weakcall(method, invocation),
            _ => Err(self.raise(Error::Type, "value is not callable")),
        }
    }

    fn invocation(
        &mut self,
        id: usize,
        invocation: Invocation,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let closure = &self.closures[id];
        let frame = Frame::direct(invocation.receiver, closure.home.clone());
        let scope = self.context(closure.scope, frame);
        self.body((id, scope), invocation.arguments, host)
    }

    pub(crate) fn body(
        &mut self,
        location: (usize, usize),
        arguments: Vec<Value>,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let code = self.closures[location.0].code.clone();
        if code.asynchronous {
            let promise = self.promise()?;
            let result = self.parameters(&code.params, (location.1, arguments), host);
            if let Err(failure) = result {
                self.finish(&promise, Err(failure), host)?;
                return Ok(promise);
            }
            let scope = self.scope(location.1);
            let task = super::flow::Task::new(promise.clone(), code.body, scope);
            self.resume(task, Ok(Value::Undefined), host)?;
            return Ok(promise);
        }
        self.parameters(&code.params, (location.1, arguments), host)?;
        let scope = self.scope(location.1);
        match self.block(&code.body, scope, host)? {
            Flow::Return(value) => Ok(value),
            Flow::Next(_) => Ok(Value::Undefined),
            Flow::Throw(value) => Err(Failure::Throw(value)),
            _ => Err(Fault("loop control escaped function".into()).into()),
        }
    }
}
