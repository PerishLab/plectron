mod resources;
use resources::Resources;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Resource(Arc<usize>);
impl Resource {
    pub fn new(bytes: usize) -> Self {
        Self(Arc::new(bytes))
    }
    pub fn bytes(&self) -> usize {
        *self.0
    }
}
impl PartialEq for Resource {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for Resource {}
impl Hash for Resource {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

#[derive(Debug, Clone, Default)]
pub struct Cost {
    overflow: bool,
    pub bytes: usize,
    resources: Resources,
}
impl Cost {
    pub fn new(bytes: usize) -> Self {
        Self {
            bytes,
            overflow: false,
            resources: Resources::default(),
        }
    }
    pub fn resource(&mut self, resource: Resource) {
        self.resources.insert(resource);
    }
    pub fn extend(&mut self, other: &Self) {
        self.overflow |= other.overflow || self.bytes.checked_add(other.bytes).is_none();
        self.bytes = self.bytes.saturating_add(other.bytes);
        self.resources.extend(other.resources.iter().cloned());
    }
    pub(super) fn checked(&self) -> Option<usize> {
        if self.overflow {
            return None;
        }
        self.resources
            .iter()
            .try_fold(self.bytes, |bytes, resource| {
                bytes.checked_add(resource.bytes())
            })
    }
    pub fn total(&self) -> usize {
        self.resources.iter().fold(self.bytes, |bytes, resource| {
            bytes.saturating_add(resource.bytes())
        })
    }
    pub fn share(mut self) -> Self {
        let bytes = std::mem::take(&mut self.bytes);
        self.resource(Resource::new(bytes));
        self
    }
}

#[derive(Default)]
pub(super) struct Ledger {
    entries: HashMap<Resource, usize, ahash::RandomState>,
    local: usize,
    shared: usize,
}
impl Ledger {
    pub fn extra(&self, cost: &Cost) -> usize {
        cost.resources
            .iter()
            .filter(|resource| !self.entries.contains_key(*resource))
            .fold(cost.bytes, |bytes, resource| {
                bytes.saturating_add(resource.bytes())
            })
    }
    pub fn retain(&mut self, cost: &Cost) {
        self.local += cost.bytes;
        for resource in cost.resources.iter() {
            let count = self.entries.entry(resource.clone()).or_default();
            if *count == 0 {
                self.shared += resource.bytes();
            }
            *count += 1;
        }
    }
    pub fn release(&mut self, cost: &Cost) {
        self.local -= cost.bytes;
        for resource in cost.resources.iter() {
            let count = self.entries.get_mut(resource).expect("charged resource");
            *count -= 1;
            if *count == 0 {
                self.entries.remove(resource);
                self.shared -= resource.bytes();
            }
        }
    }
    pub fn publish(&self, stats: &mut super::Stats) {
        stats.local = self.local;
        stats.shared = self.shared;
        stats.resources = self.entries.len();
        stats.bytes = self.local + self.shared;
    }
}
