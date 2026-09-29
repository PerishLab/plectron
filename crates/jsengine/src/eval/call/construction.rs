use crate::scope::Frame;
use crate::syntax::Item;
use crate::value::Construct;
use crate::{Addons, Engine, Error, Failure, Fault, Value};

struct Construction {
    callee: Value,
    target: Value,
    arguments: Vec<Value>,
}

impl Engine {
    pub(crate) fn build(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.create(
            Construction {
                target: callee.clone(),
                callee,
                arguments,
            },
            host,
        )
    }

    pub(crate) fn buildtarget(
        &mut self,
        callee: Value,
        target: Value,
        arguments: Vec<Value>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.create(
            Construction {
                callee,
                target,
                arguments,
            },
            host,
        )
    }

    pub(crate) fn construction(&mut self, callee: &Value) -> Result<Construct, Failure> {
        self.validate(callee)?;
        let construct = match callee {
            Value::Object(handle) => self.objects[handle.slot].construct,
            _ => None,
        };
        construct.ok_or_else(|| self.raise(Error::Type, "value is not a constructor"))
    }

    fn create(&mut self, request: Construction, host: &mut impl Addons) -> Result<Value, Failure> {
        self.tick()?;
        self.validate(&request.callee)?;
        self.validate(&request.target)?;
        for value in &request.arguments {
            self.validate(value)?;
        }
        let construct = self.construction(&request.callee)?;
        self.construction(&request.target)?;
        if self.depth >= crate::DEPTH {
            return Err(Fault("evaluation depth exhausted".into()).into());
        }
        self.depth += 1;
        let result = self.constructing(construct, request, host);
        self.depth -= 1;
        result
    }

    fn constructing(
        &mut self,
        construct: Construct,
        request: Construction,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        match construct {
            Construct::Factory => {
                let target = request
                    .arguments
                    .first()
                    .cloned()
                    .unwrap_or(Value::Undefined);
                let handler = request
                    .arguments
                    .get(1)
                    .cloned()
                    .unwrap_or(Value::Undefined);
                self.proxy(&target, &handler)
            }
            Construct::Array => self.arraycall(&request.arguments),
            Construct::Weak(kind) => self.weakbuild(kind, request.target, request.arguments, host),
            Construct::Proxy => {
                let proxy = self.proxydata(&request.callee).unwrap();
                self.proxybuild(&proxy, request.target, request.arguments, host)
            }
            Construct::Promise => {
                if request.target != request.callee {
                    return Err(Fault("Promise subclass construction is unsupported".into()).into());
                }
                self.executor(request.arguments, host)
            }
            Construct::Error => {
                let value = self.creating(&request.target, host)?;
                self.message(&value, &request.arguments)?;
                Ok(value)
            }
            Construct::Closure(slot) => self.activation((slot, false), request, host),
            Construct::Derived(Some(slot)) => self.activation((slot, true), request, host),
            Construct::Derived(None) => {
                let parent = self.parenting(&request.callee, host)?;
                let value = self.create(
                    Construction {
                        callee: parent,
                        target: request.target,
                        arguments: request.arguments,
                    },
                    host,
                )?;
                self.populate(&value, &self.initializers(&request.callee), host)?;
                Ok(value)
            }
        }
    }

    fn activation(
        &mut self,
        implementation: (usize, bool),
        request: Construction,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let (slot, derived) = implementation;
        let receiver = if derived {
            None
        } else {
            Some(self.creating(&request.target, host)?)
        };
        let closure = &self.closures[slot];
        let frame = Frame {
            receiver: receiver.clone(),
            target: request.target,
            home: closure.home.clone(),
            constructor: Some(request.callee.clone()),
        };
        let scope = self.context(closure.scope, frame);
        if let Some(receiver) = &receiver {
            self.populate(receiver, &self.initializers(&request.callee), host)?;
        }
        let value = self.body((slot, scope), request.arguments, host)?;
        if matches!(value, Value::Object(_)) {
            return Ok(value);
        }
        if derived && value != Value::Undefined {
            return Err(self.raise(Error::Type, "derived constructor returned a primitive"));
        }
        self.receiver(scope)
    }

    pub(crate) fn parent(
        &mut self,
        arguments: &[Item],
        scope: usize,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let frame = self
            .frame(scope)
            .cloned()
            .ok_or_else(|| Fault("missing super context".into()))?;
        let constructor = frame
            .constructor
            .ok_or_else(|| Fault("missing constructor".into()))?;
        let callee = self.parenting(&constructor, host)?;
        let arguments = self.arguments(arguments, scope, host)?;
        let value = self.create(
            Construction {
                callee,
                target: frame.target,
                arguments,
            },
            host,
        )?;
        self.bind(scope, value.clone())?;
        self.populate(&value, &self.initializers(&constructor), host)?;
        Ok(value)
    }
}
