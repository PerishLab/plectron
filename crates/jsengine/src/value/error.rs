use super::{Call, Construct};
use crate::{Engine, Failure, Fault, Property, Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    Error,
    Type,
    Reference,
    Syntax,
    Range,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Error => "Error",
            Self::Type => "TypeError",
            Self::Reference => "ReferenceError",
            Self::Syntax => "SyntaxError",
            Self::Range => "RangeError",
        }
    }
}

impl Engine {
    pub(crate) fn builtins(&mut self) -> Result<(), Fault> {
        for kind in [
            Kind::Error,
            Kind::Type,
            Kind::Reference,
            Kind::Syntax,
            Kind::Range,
        ] {
            let parent = match self.errors.get(&Kind::Error) {
                Some(error) => Some(self.get(error, "prototype")?),
                None => None,
            };
            let prototype = self.object(BTreeMap::from([
                ("name".into(), Value::Text(kind.name().into())),
                ("message".into(), Value::Text(String::new())),
            ]))?;
            let constructor = self.constructor(Call::Error, Construct::Error, prototype.clone());
            self.signature(&constructor, kind.name(), 1);
            let Value::Object(handle) = &prototype else {
                unreachable!()
            };
            for property in self.objects[handle.slot].fields.values() {
                property.enumerable = false;
            }
            if let Some(parent) = self.errors.get(&Kind::Error).cloned() {
                self.inherit(&constructor, Some(&parent));
            }
            self.inherit(&prototype, parent.as_ref());
            self.register(kind.name(), constructor.clone())?;
            self.errors.insert(kind, constructor);
        }
        Ok(())
    }

    pub fn exception(&mut self, kind: Kind, message: Option<String>) -> Result<Value, Fault> {
        let constructor = self.errors[&kind].clone();
        let value = self.creation(&constructor)?;
        if let Some(message) = message {
            let Value::Object(handle) = &value else {
                unreachable!()
            };
            self.objects[handle.slot]
                .fields
                .insert("message".into(), Property::hidden(Value::Text(message)));
        }
        Ok(value)
    }

    pub(crate) fn raise(&mut self, kind: Kind, message: impl Into<String>) -> Failure {
        match self.exception(kind, Some(message.into())) {
            Ok(value) => Failure::Throw(value),
            Err(fault) => Failure::Fault(fault),
        }
    }

    pub fn diagnostic(&self, failure: &Failure) -> Fault {
        match failure {
            Failure::Fault(fault) => fault.clone(),
            Failure::Throw(value) => Fault(format!("uncaught exception: {}", self.describe(value))),
        }
    }

    fn describe(&self, value: &Value) -> String {
        match (self.get(value, "name"), self.get(value, "message")) {
            (Ok(Value::Text(name)), Ok(Value::Text(message))) => format!("{name}: {message}"),
            _ => value.text(),
        }
    }

    pub(crate) fn message(&mut self, receiver: &Value, arguments: &[Value]) -> Result<(), Failure> {
        match arguments.first() {
            None | Some(Value::Undefined) => Ok(()),
            Some(Value::Object(_)) => Err(Fault("object coercion is unsupported".into()).into()),
            Some(value) => self.property(
                receiver,
                "message",
                Property::hidden(Value::Text(value.text())),
            ),
        }
    }
}
