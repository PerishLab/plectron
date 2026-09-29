use super::elements::{Elements, Fields, LIMIT, index};
use super::property::Proxy;
use super::{Atom, Call, Construct, Content, Initializer};
use crate::{Engine, Fault, Handle, Property, Value};
use std::collections::BTreeMap;

pub(crate) struct Object {
    pub fields: Fields,
    pub(super) length: Option<u32>,
    elements: Option<Box<Elements>>,
    pub frozen: bool,
    pub extensible: bool,
    pub prototype: Option<Handle>,
    pub call: Option<Call>,
    pub construct: Option<Construct>,
    pub initializers: Vec<Initializer>,
    pub proxy: Option<Proxy>,
    pub iterator: Option<super::access::State>,
}

impl Object {
    pub(crate) fn edges(&self) -> impl Iterator<Item = &Property> {
        self.fields
            .iter()
            .map(|(_, property)| property)
            .chain(self.elements.iter().flat_map(|elements| elements.iter()))
    }

    pub fn new(fields: BTreeMap<String, Value>) -> Self {
        Self {
            fields: Fields::new(fields),
            length: None,
            elements: None,
            frozen: false,
            extensible: true,
            prototype: None,
            call: None,
            construct: None,
            initializers: Vec::new(),
            proxy: None,
            iterator: None,
        }
    }

    fn resize(&mut self, value: &Value) -> Result<(), Fault> {
        let number = value.number()?;
        let length = number as u32;
        self.fields.retain(|name, _| {
            name.string()
                .and_then(index)
                .is_none_or(|index| index < length)
        });
        self.length = Some(length);
        if let Some(elements) = &mut self.elements {
            elements.truncate(length);
        }
        Ok(())
    }

    pub(super) fn set(&mut self, name: &Atom, value: Value) -> Result<(), Fault> {
        if self.length.is_some() && name.string() == Some("length") {
            return self.resize(&value);
        }
        if let Some(property) = self
            .elements
            .as_mut()
            .and_then(|elements| name.string().and_then(|name| elements.mutable(name)))
            .or_else(|| self.fields.mutable(name))
        {
            let Content::Data { value: current, .. } = &mut property.content else {
                return Err(Fault("accessor requires an execution context".into()));
            };
            *current = value;
            return Ok(());
        }
        self.store(name, Property::data(value))
    }

    pub(super) fn store(&mut self, name: &Atom, property: Property) -> Result<(), Fault> {
        self.check(name)?;
        if !self.extensible && self.lookup(name).is_none() {
            return Err(Fault("object is not extensible".into()));
        }
        if let (Some(length), Some(index)) = (&mut self.length, name.string().and_then(index)) {
            *length = (*length).max(index + 1);
        }
        let property = if let Some(elements) = &mut self.elements {
            let stored = if let Some(name) = name.string() {
                elements.store(name, property)
            } else {
                Err(property)
            };
            match stored {
                Ok(()) => {
                    self.fields.remove(name);
                    return Ok(());
                }
                Err(property) => property,
            }
        } else {
            property
        };
        self.fields.insert(name.clone(), property);
        Ok(())
    }

    pub(super) fn check(&self, name: &Atom) -> Result<(), Fault> {
        let count = self.fields.len()
            + self
                .elements
                .as_ref()
                .map_or(0, |elements| elements.count());
        if name.string().is_some_and(|name| name.len() > LIMIT)
            || (count >= LIMIT && self.lookup(name).is_none())
        {
            return Err(Fault("object property budget exhausted".into()));
        }
        Ok(())
    }

    pub(crate) fn values(&self) -> Result<BTreeMap<String, Value>, Fault> {
        let mut values: BTreeMap<_, _> = self
            .fields
            .iter()
            .filter_map(|(name, property)| name.string().map(|name| (name, property)))
            .map(|(name, property)| Ok((name.to_owned(), property.value()?)))
            .collect::<Result<_, Fault>>()?;
        if let Some(elements) = &self.elements {
            elements.values(&mut values)?;
        }
        if let Some(length) = self.length {
            values.insert("length".into(), Value::Number(length.into()));
        }
        Ok(values)
    }

    pub(super) fn own(&self, name: &str) -> Option<Property> {
        self.lookup(&name.into())
    }

    pub(super) fn lookup(&self, name: &Atom) -> Option<Property> {
        if let Some(length) = self.length
            && name.string() == Some("length")
        {
            return Some(Property::fixed(Value::Number(length.into()), true));
        }
        self.elements
            .as_ref()
            .and_then(|elements| name.string().and_then(|name| elements.get(name)))
            .or_else(|| self.fields.get(name))
            .cloned()
    }
    pub(super) fn content(&self, name: &Atom) -> Option<Content> {
        if let Some(length) = self.length
            && name.string() == Some("length")
        {
            return Some(Content::Data {
                value: Value::Number(length.into()),
                writable: true,
            });
        }
        self.elements
            .as_ref()
            .and_then(|elements| name.string().and_then(|name| elements.get(name)))
            .or_else(|| self.fields.get(name))
            .map(|property| property.content.clone())
    }
    pub(super) fn remove(&mut self, name: &Atom) -> Option<Property> {
        self.elements
            .as_mut()
            .and_then(|elements| name.string().and_then(|name| elements.remove(name)))
            .or_else(|| self.fields.remove(name))
    }
    pub(crate) fn properties(&self) -> BTreeMap<String, Property> {
        let mut fields = self
            .fields
            .iter()
            .filter_map(|(name, property)| {
                name.string()
                    .map(|name| (name.to_owned(), property.clone()))
            })
            .collect();
        if let Some(elements) = &self.elements {
            elements.publish(&mut fields);
        }
        fields
    }

    pub(super) fn keys(&self) -> Vec<Atom> {
        let mut indices: Vec<u32> = self
            .fields
            .iter()
            .filter_map(|(name, _)| name.string().and_then(index))
            .collect();
        if let Some(elements) = &self.elements {
            indices.extend(elements.keys());
        }
        indices.sort_unstable();
        indices.dedup();
        let mut keys: Vec<_> = indices
            .into_iter()
            .map(|index| Atom::String(index.to_string()))
            .collect();
        if self.length.is_some() {
            keys.push(Atom::String("length".into()));
        }
        keys.extend(self.fields.iter().filter_map(|(name, _)| {
            name.string()
                .filter(|name| index(name).is_none())
                .map(|_| name.clone())
        }));
        keys.extend(
            self.fields
                .iter()
                .filter(|(name, _)| matches!(name, Atom::Symbol(_)))
                .map(|(name, _)| name.clone()),
        );
        keys
    }
}

impl Engine {
    pub fn keys(&self, object: &Value) -> Result<Vec<Atom>, Fault> {
        self.guard(object)?;
        let Value::Object(handle) = object else {
            return Err(Fault("expected object".into()));
        };
        Ok(self.objects[handle.slot].keys())
    }

    pub fn freeze(&mut self, object: &Value) -> Result<(), Fault> {
        self.guard(object)?;
        let Value::Object(handle) = object else {
            return Err(Fault("expected object".into()));
        };
        self.objects[handle.slot].frozen = true;
        self.objects[handle.slot].extensible = false;
        Ok(())
    }

    pub fn array(&mut self, items: Vec<Option<Value>>) -> Result<Value, Fault> {
        if items.len() > LIMIT {
            return Err(Fault("array literal budget exhausted".into()));
        }
        let length = items.len() as u32;
        for item in items.iter().flatten() {
            self.validate(item)?;
        }
        let value = self.object(BTreeMap::new())?;
        let Value::Object(handle) = &value else {
            unreachable!()
        };
        self.objects[handle.slot].length = Some(length);
        self.objects[handle.slot].prototype = Some(self.roots.array);
        self.objects[handle.slot].elements = Some(Box::new(Elements::new(items)));
        Ok(value)
    }
}

impl Value {
    pub(crate) fn key(&self) -> Result<Atom, Fault> {
        match self {
            Self::Object(_) => Err(Fault("object property key coercion is unsupported".into())),
            Self::Symbol(symbol) => Ok(Atom::Symbol(*symbol)),
            _ => Ok(Atom::String(self.text())),
        }
    }
}
