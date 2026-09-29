use super::super::envelope::Envelope;
use crate::shared::{Fault, Item, Output};
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy, Snapshot};
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, Weak},
};

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 2048,
    bytes: 8_388_608,
};
const GROUPS: usize = 32;
const ITEMS: usize = 128;

struct Key {
    items: Weak<[Item]>,
    scale: u64,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.items, &other.items) && self.scale == other.scale
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.items.as_ptr().cast::<Item>().hash(state);
        self.scale.hash(state);
    }
}

pub(in crate::shared::preparation) struct Trees {
    cache: Cache<Key, Option<Envelope>>,
    manager: Manager,
    config: config::Store,
    setting: Option<config::Value>,
    bounds: Option<config::Value>,
    enabled: bool,
    groups: usize,
    pub items: usize,
}
impl Trees {
    pub fn new(config: config::Store) -> Self {
        let manager = Manager::default();
        Self {
            cache: manager
                .register("enclosures", POLICY)
                .expect("fresh subtree cache"),
            manager,
            config,
            setting: None,
            bounds: None,
            enabled: true,
            groups: GROUPS,
            items: ITEMS,
        }
    }
    pub fn maintain(&mut self) -> Result<(), Fault> {
        let bounds = self.config.get("render.bounds");
        if bounds != self.bounds {
            self.clear();
            self.bounds = bounds;
        }
        self.enabled = self
            .config
            .get("render.subtrees")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config render.subtrees: {error}")))?
            .unwrap_or(true);
        self.groups = self
            .config
            .get("render.enclosure.groups")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config render.enclosure.groups: {error}")))?
            .unwrap_or(GROUPS);
        let items = self
            .config
            .get("render.enclosure.items")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config render.enclosure.items: {error}")))?
            .unwrap_or(ITEMS);
        if items != self.items {
            self.cache.clear();
            self.items = items;
        }
        let setting = self.config.get("render.enclosures");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config render.enclosures: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.setting = setting;
        }
        self.cache.maintain();
        Ok(())
    }
    pub fn accepts(&self, groups: usize) -> bool {
        if !self.enabled || !(1..=self.groups).contains(&groups) {
            return false;
        }
        self.cache.reading() || self.cache.writing()
    }
    pub fn clear(&mut self) {
        self.cache.clear();
    }
    pub fn get(&mut self, items: &Arc<[Item]>, output: Output) -> Option<Option<Envelope>> {
        self.cache.get(&Key {
            items: Arc::downgrade(items),
            scale: output.scale().to_bits(),
        })
    }
    pub fn store(&mut self, items: &Arc<[Item]>, output: Output, envelope: Option<Envelope>) {
        self.cache.insert(
            Key {
                items: Arc::downgrade(items),
                scale: output.scale().to_bits(),
            },
            envelope,
            std::mem::size_of::<Key>()
                + std::mem::size_of::<Option<Envelope>>()
                + std::mem::size_of_val(items.as_ref()),
        );
    }
    pub fn inspect(&self) -> Snapshot {
        self.cache.publish();
        self.manager.inspect().remove(0)
    }
}
