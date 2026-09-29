use super::{Binding, Fault, Stroke};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Clone)]
pub struct Handle {
    owner: Arc<()>,
    id: u64,
}

#[derive(Debug, Clone)]
pub struct Set<C> {
    pub priority: i32,
    pub bindings: Vec<Binding<C>>,
}
impl<C> Set<C> {
    fn validate(&self) -> Result<(), Fault> {
        if self.bindings.len() > 64 {
            return Err(Fault::Budget);
        }
        for binding in &self.bindings {
            binding.chord.validate()?;
        }
        Ok(())
    }
}

pub struct Scope<C> {
    owner: Arc<()>,
    next: u64,
    groups: BTreeMap<u64, Set<C>>,
}
impl<C> Default for Scope<C> {
    fn default() -> Self {
        Self {
            owner: Arc::new(()),
            next: 0,
            groups: BTreeMap::new(),
        }
    }
}
impl<C> Scope<C> {
    pub fn register(&mut self, set: Set<C>) -> Result<Handle, Fault> {
        set.validate()?;
        if self.groups.len() >= 32 {
            return Err(Fault::Budget);
        }
        let id = self.next.checked_add(1).ok_or(Fault::Budget)?;
        self.next = id;
        self.groups.insert(id, set);
        Ok(Handle {
            owner: self.owner.clone(),
            id,
        })
    }

    fn check(&self, handle: &Handle) -> Result<(), Fault> {
        if Arc::ptr_eq(&self.owner, &handle.owner) && self.groups.contains_key(&handle.id) {
            Ok(())
        } else {
            Err(Fault::Handle)
        }
    }

    pub fn replace(&mut self, handle: &Handle, set: Set<C>) -> Result<(), Fault> {
        self.check(handle)?;
        set.validate()?;
        self.groups.insert(handle.id, set);
        Ok(())
    }

    pub fn remove(&mut self, handle: &Handle) -> Result<(), Fault> {
        self.check(handle)?;
        self.groups.remove(&handle.id);
        Ok(())
    }

    pub fn candidates(&self, stroke: &Stroke) -> Vec<Binding<C>>
    where
        C: Clone,
    {
        let mut groups = self.groups.iter().collect::<Vec<_>>();
        groups.sort_by_key(|(id, set)| (std::cmp::Reverse(set.priority), **id));
        groups
            .into_iter()
            .flat_map(|(_, set)| &set.bindings)
            .filter(|binding| binding.chord == stroke.chord && (!stroke.repeat || binding.repeat))
            .cloned()
            .collect()
    }
}
