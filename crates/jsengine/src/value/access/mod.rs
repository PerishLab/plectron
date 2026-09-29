mod array;
mod iterator;
mod operations;
mod ordinary;
pub(crate) use iterator::State;

use crate::{Addons, Content, Engine, Error, Failure, Invocation, Value, value::Atom};

#[derive(Debug, Clone)]
pub struct Access {
    pub base: Value,
    pub receiver: Value,
    pub key: String,
}

pub(crate) struct View<'a> {
    pub base: &'a Value,
    pub receiver: &'a Value,
    pub key: Atom,
}

impl Access {
    fn view(&self) -> View<'_> {
        View {
            base: &self.base,
            receiver: &self.receiver,
            key: self.key.clone().into(),
        }
    }
    pub fn direct(value: &Value, key: impl Into<String>) -> Self {
        Self {
            base: value.clone(),
            receiver: value.clone(),
            key: key.into(),
        }
    }
}

impl Engine {
    pub fn read(&mut self, access: &Access, host: &mut impl Addons) -> Result<Value, Failure> {
        self.reading(access.view(), host)
    }

    pub(crate) fn reading(
        &mut self,
        access: View<'_>,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.activity.reads = self.activity.reads.saturating_add(1);
        self.enter();
        let result = self.retrieve(&access, host);
        self.turns -= 1;
        result
    }

    pub fn write(
        &mut self,
        access: &Access,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        self.writing(access.view(), value, host)
    }

    pub(crate) fn writing(
        &mut self,
        access: View<'_>,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        if self.attempting(access, value, host)? {
            Ok(())
        } else {
            Err(self.raise(Error::Type, "property assignment failed"))
        }
    }

    pub fn attempt(
        &mut self,
        access: &Access,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.attempting(access.view(), value, host)
    }

    pub(crate) fn attempting(
        &mut self,
        access: View<'_>,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.activity.writes = self.activity.writes.saturating_add(1);
        self.enter();
        let result = self.update(&access, value, host);
        self.turns -= 1;
        result
    }

    fn access(&mut self, access: &View<'_>) -> Result<(), Failure> {
        self.tick()?;
        self.validate(access.base)?;
        if let Atom::Symbol(symbol) = access.key {
            self.validate(&Value::Symbol(symbol))?;
        }
        if access.receiver != access.base {
            self.validate(access.receiver)?;
        }
        if !matches!(access.base, Value::Object(_) | Value::Text(_)) {
            return Err(self.raise(Error::Type, "cannot access property of non-object"));
        }
        Ok(())
    }

    fn retrieve(&mut self, access: &View<'_>, host: &mut impl Addons) -> Result<Value, Failure> {
        self.access(access)?;
        if let Value::Text(text) = access.base {
            if access.key == Atom::Symbol(self.iterator) {
                return Ok(self.textiter.clone());
            }
            return Ok(access
                .key
                .string()
                .map_or(Value::Undefined, |key| self.text(text, key)));
        }
        let mut base = access.base.clone();
        loop {
            if let Some(proxy) = self.proxydata(&base) {
                return self.proxyget(&proxy, &access.key, access.receiver, host);
            }
            match self
                .observing(&base, &access.key, host)?
                .map(|property| property.content)
            {
                Some(Content::Data { value, .. }) => return Ok(value),
                Some(Content::Accessor { get: Some(get), .. }) => {
                    return self.apply(
                        get,
                        Invocation {
                            receiver: access.receiver.clone(),
                            arguments: Vec::new(),
                        },
                        host,
                    );
                }
                Some(Content::Accessor { get: None, .. }) => return Ok(Value::Undefined),
                None => base = self.parenting(&base, host)?,
            }
            if base == Value::Null {
                return Ok(Value::Undefined);
            }
        }
    }

    fn update(
        &mut self,
        access: &View<'_>,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.validate(&value)?;
        self.access(access)?;
        if matches!(access.base, Value::Text(_)) {
            return Err(self.raise(Error::Type, "text is immutable"));
        }
        let mut base = access.base.clone();
        loop {
            if let Some(proxy) = self.proxydata(&base) {
                return self.proxyset(&proxy, &access.key, (value, access.receiver), host);
            }
            match self
                .observing(&base, &access.key, host)?
                .map(|property| property.content)
            {
                Some(Content::Accessor { set: Some(set), .. }) => {
                    self.apply(
                        set,
                        Invocation {
                            receiver: access.receiver.clone(),
                            arguments: vec![value],
                        },
                        host,
                    )?;
                    return Ok(true);
                }
                Some(Content::Accessor { set: None, .. }) => return Ok(false),
                Some(Content::Data {
                    writable: false, ..
                }) => return Ok(false),
                Some(Content::Data { writable: true, .. }) => {
                    return self.saving(access.receiver, &access.key, value, host);
                }
                None => base = self.parenting(&base, host)?,
            }
            if base == Value::Null {
                return self.saving(access.receiver, &access.key, value, host);
            }
        }
    }
}
