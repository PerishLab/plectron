use crate::{Property, Value, value::Atom};
use std::collections::BTreeMap;

pub(super) const LIMIT: usize = 4096;

#[derive(Clone, Default)]
pub(crate) struct Fields(Vec<(Atom, Property)>);

impl Fields {
    pub fn new(fields: BTreeMap<String, Value>) -> Self {
        if fields.is_empty() {
            return Self(Vec::with_capacity(2));
        }
        Self(
            fields
                .into_iter()
                .map(|(name, value)| (Atom::String(name), Property::data(value)))
                .collect(),
        )
    }
    pub fn get(&self, name: &Atom) -> Option<&Property> {
        self.0
            .iter()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
    pub fn mutable(&mut self, name: &Atom) -> Option<&mut Property> {
        self.0
            .iter_mut()
            .rev()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value)
    }
    pub fn insert(&mut self, name: Atom, property: Property) -> Option<Property> {
        if let Some(current) = self.mutable(&name) {
            return Some(std::mem::replace(current, property));
        }
        self.0.push((name, property));
        None
    }
    pub fn remove(&mut self, name: &Atom) -> Option<Property> {
        let index = self.0.iter().position(|(key, _)| key == name)?;
        Some(self.0.remove(index).1)
    }
    pub fn retain(&mut self, mut keep: impl FnMut(&Atom, &mut Property) -> bool) {
        self.0.retain_mut(|(name, property)| keep(name, property));
    }
    pub fn iter(&self) -> impl Iterator<Item = (&Atom, &Property)> {
        self.0.iter().map(|(name, property)| (name, property))
    }
    pub fn values(&mut self) -> impl Iterator<Item = &mut Property> {
        self.0.iter_mut().map(|(_, property)| property)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Default)]
pub(super) struct Elements {
    slots: Vec<Option<Property>>,
    count: usize,
}
impl Elements {
    pub fn iter(&self) -> impl Iterator<Item = &Property> {
        self.slots.iter().flatten()
    }

    pub fn new(values: Vec<Option<Value>>) -> Self {
        Self {
            count: values.iter().flatten().count(),
            slots: values
                .into_iter()
                .map(|value| value.map(Property::data))
                .collect(),
        }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn get(&self, name: &str) -> Option<&Property> {
        self.slots.get(index(name)? as usize)?.as_ref()
    }
    pub fn mutable(&mut self, name: &str) -> Option<&mut Property> {
        self.slots.get_mut(index(name)? as usize)?.as_mut()
    }
    pub fn remove(&mut self, name: &str) -> Option<Property> {
        let old = self.slots.get_mut(index(name)? as usize)?.take()?;
        self.count -= 1;
        Some(old)
    }
    pub fn store(&mut self, name: &str, property: Property) -> Result<(), Property> {
        let Some(index) = index(name).map(|index| index as usize) else {
            return Err(property);
        };
        if index > self.slots.len() || index >= LIMIT {
            return Err(property);
        }
        if index == self.slots.len() {
            self.slots.push(None);
        }
        if self.slots[index].replace(property).is_none() {
            self.count += 1;
        }
        Ok(())
    }
    pub fn truncate(&mut self, length: u32) {
        let length = (length as usize).min(self.slots.len());
        self.count -= self.slots[length..].iter().flatten().count();
        self.slots.truncate(length);
    }
    pub fn publish(&self, fields: &mut BTreeMap<String, Property>) {
        for (index, property) in self.slots.iter().enumerate() {
            if let Some(property) = property {
                fields.insert(index.to_string(), property.clone());
            }
        }
    }
    pub fn keys(&self) -> impl Iterator<Item = u32> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(index, property)| property.as_ref().map(|_| index as u32))
    }
    pub fn values(&self, values: &mut BTreeMap<String, Value>) -> Result<(), crate::Fault> {
        for (index, property) in self.slots.iter().enumerate() {
            if let Some(property) = property {
                values.insert(index.to_string(), property.value()?);
            }
        }
        Ok(())
    }
}
pub(super) fn index(name: &str) -> Option<u32> {
    if !name.as_bytes().first()?.is_ascii_digit() || (name.len() > 1 && name.starts_with('0')) {
        return None;
    }
    name.parse::<u32>().ok().filter(|index| *index < u32::MAX)
}
