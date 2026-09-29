use super::{Engine, Expression, Host, Value};
use crate::{
    Error, Failure, Fault, Invocation,
    value::{Atom, View},
};
use std::borrow::Cow;

pub(super) enum Reference<'a> {
    Binding {
        address: Option<crate::scope::Address>,
        name: Cow<'a, str>,
    },
    Property {
        base: Value,
        key: Key<'a>,
        receiver: Value,
    },
}

pub(super) enum Key<'a> {
    Text(&'a str),
    Value(Value),
}

impl Key<'_> {
    fn owned(self) -> Key<'static> {
        Key::Value(match self {
            Self::Text(text) => Value::Text(text.into()),
            Self::Value(value) => value,
        })
    }
}

impl Reference<'_> {
    pub fn owned(self) -> Reference<'static> {
        match self {
            Self::Binding { address, name } => Reference::Binding {
                address,
                name: Cow::Owned(name.into_owned()),
            },
            Self::Property {
                base,
                key,
                receiver,
            } => Reference::Property {
                base,
                key: key.owned(),
                receiver,
            },
        }
    }

    fn key(engine: &mut Engine, base: &Value, key: &Key<'_>) -> Result<Atom, Failure> {
        if matches!(base, Value::Null | Value::Undefined) {
            return Err(engine.raise(Error::Type, "cannot access property of null or undefined"));
        }
        Ok(match key {
            Key::Text(text) => Atom::String((*text).into()),
            Key::Value(value) => {
                engine.validate(value)?;
                value.key()?
            }
        })
    }

    pub fn receiver(&self) -> Value {
        match self {
            Self::Binding { .. } => Value::Undefined,
            Self::Property { receiver, .. } => receiver.clone(),
        }
    }

    pub fn get(&self, engine: &mut Engine, host: &mut impl Host) -> Result<Value, Failure> {
        match self {
            Self::Binding {
                address: Some(address),
                name,
            } => engine.load(*address, name),
            Self::Binding {
                address: None,
                name,
            } => Err(engine.raise(Error::Reference, format!("unknown binding: {name}"))),
            Self::Property {
                base,
                key,
                receiver,
            } => {
                let key = Self::key(engine, base, key)?;
                engine.reading(
                    View {
                        base,
                        receiver,
                        key,
                    },
                    host,
                )
            }
        }
    }

    pub fn set(
        self,
        engine: &mut Engine,
        value: Value,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        match self {
            Self::Binding {
                address: Some(address),
                name,
            } => engine.assign(address, &name, value),
            Self::Binding {
                address: None,
                name,
            } => Err(engine.raise(Error::Reference, format!("unknown binding: {name}"))),
            Self::Property {
                base,
                key,
                receiver,
            } => {
                let key = Self::key(engine, &base, &key)?;
                engine.writing(
                    View {
                        base: &base,
                        receiver: &receiver,
                        key,
                    },
                    value.clone(),
                    host,
                )?;
                Ok(value)
            }
        }
    }

    pub fn delete(self, engine: &mut Engine, host: &mut impl Host) -> Result<Value, Failure> {
        let Self::Property { base, key, .. } = self else {
            return Err(engine.raise(Error::Syntax, "strict delete target is forbidden"));
        };
        let key = Self::key(engine, &base, &key)?;
        let deleted = engine.removing(&base, &key, host)?;
        if !deleted {
            return Err(engine.raise(Error::Type, "property is not configurable"));
        }
        Ok(Value::Boolean(true))
    }
}

impl Engine {
    pub(super) fn reference<'a>(
        &mut self,
        expression: &'a Expression,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Reference<'a>, Failure> {
        match expression {
            Expression::Name(name) => Ok(Reference::Binding {
                address: self.locate(scope, name),
                name: Cow::Borrowed(name),
            }),
            Expression::Member(object, key) => {
                let base = self.evaluate(object, scope, host)?;
                let key = self.key(key, scope, host)?;
                Ok(Reference::Property {
                    receiver: base.clone(),
                    base,
                    key,
                })
            }
            Expression::Super(key) => {
                let receiver = self.receiver(scope)?;
                let key = self.key(key, scope, host)?;
                let home = self
                    .frame(scope)
                    .and_then(|frame| frame.home.as_ref())
                    .ok_or_else(|| Fault("missing super home".into()))?;
                let home = home.clone();
                let base = self.parenting(&home, host)?;
                Ok(Reference::Property {
                    base,
                    key,
                    receiver,
                })
            }
            _ => Err(Fault("expression has no reference".into()).into()),
        }
    }

    fn key<'a>(
        &mut self,
        expression: &'a Expression,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Key<'a>, Failure> {
        match expression {
            Expression::Text(text) => self.evaluating(|_| Ok(Key::Text(text))),
            _ => self.evaluate(expression, scope, host).map(Key::Value),
        }
    }

    pub(super) fn calling(
        &mut self,
        pair: (&Expression, &[crate::syntax::Item]),
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let (callee, receiver) = match pair.0 {
            Expression::Name(_) | Expression::Member(..) | Expression::Super(_) => {
                let reference = self.reference(pair.0, scope, host)?;
                (reference.get(self, host)?, reference.receiver())
            }
            expression => (self.evaluate(expression, scope, host)?, Value::Undefined),
        };
        let arguments = self.arguments(pair.1, scope, host)?;
        self.apply(
            callee,
            Invocation {
                receiver,
                arguments,
            },
            host,
        )
    }
}
