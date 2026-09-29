use crate::{
    Addons, Engine, Error, Failure, Property, Value,
    value::{Atom, property::Descriptor},
};

impl Engine {
    pub(crate) fn checking(
        &mut self,
        object: &Value,
        key: &Atom,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.tick()?;
        self.validate(object)?;
        if let Atom::Symbol(symbol) = key {
            self.validate(&Value::Symbol(*symbol))?;
        }
        if !matches!(object, Value::Object(_)) {
            return Err(self.raise(Error::Type, "right operand of in must be an object"));
        }
        let mut current = object.clone();
        loop {
            if let Some(proxy) = self.proxydata(&current) {
                return self.proxyhas(&proxy, key, host);
            }
            if self.observing(&current, key, host)?.is_some() {
                return Ok(true);
            }
            current = self.parenting(&current, host)?;
            if current == Value::Null {
                return Ok(false);
            }
        }
    }

    pub(crate) fn removing(
        &mut self,
        object: &Value,
        key: &Atom,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        let _ = host;
        self.tick()?;
        self.validate(object)?;
        if let Atom::Symbol(symbol) = key {
            self.validate(&Value::Symbol(*symbol))?;
        }
        match object {
            Value::Object(_) => {
                if let Some(proxy) = self.proxydata(object) {
                    self.proxydelete(&proxy, key, host)
                } else {
                    self.delete(object, key)
                }
            }
            Value::Text(text) => Ok(key.string().is_none_or(|name| {
                name != "length"
                    && !name.parse::<usize>().ok().is_some_and(|index| {
                        index.to_string() == name && index < text.chars().count()
                    })
            })),
            _ => Ok(true),
        }
    }

    pub(crate) fn listing(
        &mut self,
        object: &Value,
        host: &mut impl Addons,
    ) -> Result<Vec<Atom>, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxykeys(&proxy, host)
        } else {
            Ok(self.keys(object)?)
        }
    }

    pub(crate) fn observing(
        &mut self,
        object: &Value,
        key: &Atom,
        host: &mut impl Addons,
    ) -> Result<Option<Property>, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxyobserve(&proxy, key, host)
        } else {
            Ok(self.inspect(object, key)?)
        }
    }

    pub(crate) fn testing(
        &mut self,
        object: &Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxytest(&proxy, host)
        } else {
            Ok(self.extensible(object)?)
        }
    }

    pub(crate) fn preventing(
        &mut self,
        object: &Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxyprevent(&proxy, host)
        } else {
            Ok(self.prevent(object)?)
        }
    }

    pub(crate) fn parenting(
        &mut self,
        object: &Value,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxyparent(&proxy, host)
        } else {
            Ok(self.prototype(object)?)
        }
    }

    pub(crate) fn rebasing(
        &mut self,
        object: &Value,
        parent: &Value,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxyrebase(&proxy, parent, host)
        } else {
            self.rebase(object, parent)
        }
    }

    pub(crate) fn definingown(
        &mut self,
        object: &Value,
        key: &Atom,
        descriptor: Descriptor,
        host: &mut impl Addons,
    ) -> Result<bool, Failure> {
        self.tick()?;
        if let Some(proxy) = self.proxydata(object) {
            self.proxydefine(&proxy, key, descriptor, host)
        } else {
            let current = self.observing(object, key, host)?;
            self.configure(object, key, descriptor.complete(current))
        }
    }
}
