mod descriptor;
mod invocation;

use crate::{
    Addons, Engine, Error, Failure, Property, Value,
    value::{Atom, Call, View},
};

#[derive(Clone, Copy)]
pub(crate) enum Method {
    Apply,
    Construct,
    Define,
    Delete,
    Get,
    Descriptor,
    Prototype,
    Has,
    Extensible,
    Keys,
    Prevent,
    Set,
    Rebase,
}

impl Engine {
    pub(crate) fn reflects(&mut self) -> Result<(), crate::Fault> {
        let object = self.object(Default::default())?;
        let Value::Object(handle) = object else {
            unreachable!()
        };
        for (name, method, length) in [
            ("apply", Method::Apply, 3),
            ("construct", Method::Construct, 2),
            ("defineProperty", Method::Define, 3),
            ("deleteProperty", Method::Delete, 2),
            ("get", Method::Get, 2),
            ("getOwnPropertyDescriptor", Method::Descriptor, 2),
            ("getPrototypeOf", Method::Prototype, 1),
            ("has", Method::Has, 2),
            ("isExtensible", Method::Extensible, 1),
            ("ownKeys", Method::Keys, 1),
            ("preventExtensions", Method::Prevent, 1),
            ("set", Method::Set, 3),
            ("setPrototypeOf", Method::Rebase, 2),
        ] {
            let function = self.function(Call::Reflect(method));
            self.signature(&function, name, length);
            self.objects[handle.slot]
                .fields
                .insert(name.into(), Property::hidden(function));
        }
        self.register("Reflect", object)
    }

    pub(crate) fn reflect(
        &mut self,
        method: Method,
        args: Vec<Value>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        let target = args.first().cloned().unwrap_or(Value::Undefined);
        self.validate(&target)?;
        if !matches!(target, Value::Object(_)) {
            return Err(self.raise(Error::Type, "Reflect target must be an object"));
        }
        match method {
            Method::Apply => self.applying(&target, &args, host),
            Method::Construct => self.reflectconstruct(&target, &args, host),
            Method::Define => self.defining(&target, &args, host),
            Method::Delete => {
                let key = self.reflectkey(&args)?;
                Ok(Value::Boolean(self.removing(&target, &key, host)?))
            }
            Method::Get => {
                let key = self.reflectkey(&args)?;
                let receiver = args.get(2).unwrap_or(&target);
                self.reading(
                    View {
                        base: &target,
                        receiver,
                        key,
                    },
                    host,
                )
            }
            Method::Descriptor => self.describing(&target, &args, host),
            Method::Prototype => self.parenting(&target, host),
            Method::Has => {
                let key = self.reflectkey(&args)?;
                Ok(Value::Boolean(self.checking(&target, &key, host)?))
            }
            Method::Extensible => Ok(Value::Boolean(self.testing(&target, host)?)),
            Method::Keys => {
                let keys = self.listing(&target, host)?;
                let values = keys
                    .into_iter()
                    .map(|key| match key {
                        Atom::String(value) => Value::Text(value),
                        Atom::Symbol(value) => Value::Symbol(value),
                    })
                    .map(Some)
                    .collect();
                Ok(self.array(values)?)
            }
            Method::Prevent => Ok(Value::Boolean(self.preventing(&target, host)?)),
            Method::Set => {
                let key = self.reflectkey(&args)?;
                let value = args.get(2).cloned().unwrap_or(Value::Undefined);
                let receiver = args.get(3).unwrap_or(&target);
                Ok(Value::Boolean(self.attempting(
                    View {
                        base: &target,
                        receiver,
                        key,
                    },
                    value,
                    host,
                )?))
            }
            Method::Rebase => {
                let parent = args.get(1).unwrap_or(&Value::Undefined);
                Ok(Value::Boolean(self.rebasing(&target, parent, host)?))
            }
        }
    }

    fn reflectkey(&mut self, args: &[Value]) -> Result<Atom, Failure> {
        let key = args.get(1).cloned().unwrap_or(Value::Undefined);
        self.validate(&key)?;
        Ok(key.key()?)
    }
}
