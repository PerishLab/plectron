use super::Payload;
use crate::{Fault, layout::text::paragraph::Lines};
use serde::Deserialize;
use shared::cache::{Cache as Store, Cost, Manager, Policy};
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, Weak},
};

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};

struct Key(Weak<Lines>, usize);
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0) && self.1 == other.1
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.as_ptr().hash(state);
        self.1.hash(state);
    }
}

pub(in crate::layout) struct Cache {
    values: Store<Key, Arc<[Payload]>>,
    config: config::Store,
    setting: Option<config::Value>,
    enabled: bool,
}
impl Cache {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            values: manager.register("glyphs", POLICY)?,
            config,
            setting: None,
            enabled: true,
        })
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        self.enabled = self
            .config
            .get("layout.payloads")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.payloads: {error}")))?
            .unwrap_or(true);
        let setting = self.config.get("layout.glyphs");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config layout.glyphs: {error}")))?
                .unwrap_or(POLICY);
            self.values
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.setting = setting;
        }
        Ok(())
    }
    pub(super) fn get(&mut self, lines: &Arc<Lines>, index: usize) -> Option<Arc<[Payload]>> {
        if !self.enabled || !self.values.reading() {
            return None;
        }
        self.values.get(&Key(Arc::downgrade(lines), index))
    }
    pub(super) fn store(&mut self, lines: &Arc<Lines>, index: usize, payloads: Arc<[Payload]>) {
        if !self.enabled || !self.values.writing() {
            return;
        }
        let mut cost =
            Cost::new(std::mem::size_of::<Key>() + std::mem::size_of_val(payloads.as_ref()));
        for payload in payloads.iter() {
            cost.bytes += std::mem::size_of_val(payload.glyphs.as_ref())
                + std::mem::size_of_val(payload.clusters.as_ref());
            if let Some(resource) = payload.font.resource() {
                cost.resource(resource);
            }
        }
        self.values
            .store(Key(Arc::downgrade(lines), index), payloads, cost);
    }
    pub fn maintain(&mut self) {
        self.values.maintain();
    }
    pub fn clear(&mut self) {
        self.values.clear();
    }
    pub fn close(&mut self) {
        self.values.close();
    }
    pub fn publish(&self) {
        self.values.publish();
    }
    pub fn enable(&mut self, enabled: bool) {
        let mut policy = self.values.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.values.configure(policy);
    }
}
