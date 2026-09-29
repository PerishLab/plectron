use crate::Fault;
use graphics::shared::{Font, Glyph, Ink, Record};
use serde::Deserialize;
use shared::cache::{Cache, Cost, Manager, Policy};
use std::{
    hash::{Hash, Hasher},
    sync::{Arc, Weak},
};

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 8_388_608,
};

pub(super) struct Key {
    glyphs: Weak<[Glyph]>,
    font: Font,
    color: u32,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.glyphs, &other.glyphs)
            && self.font == other.font
            && self.color == other.color
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.glyphs.as_ptr().cast::<Glyph>().hash(state);
        self.font.hash(state);
        self.color.hash(state);
    }
}

pub(super) struct Identity {
    cache: Cache<Key, Weak<Record>>,
    config: config::Store,
    setting: Option<config::Value>,
    enabled: bool,
}
impl Identity {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("records", POLICY)?,
            config,
            setting: None,
            enabled: true,
        })
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        self.enabled = self
            .config
            .get("paint.identity")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config paint.identity: {error}")))?
            .unwrap_or(true);
        let setting = self.config.get("paint.identities");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config paint.identities: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.setting = setting;
        }
        Ok(())
    }
    pub fn key(&self, ink: &Ink) -> Option<Key> {
        if !self.enabled {
            return None;
        }
        if let Ink::Text {
            font,
            glyphs,
            color,
        } = ink
        {
            Some(Key {
                glyphs: Arc::downgrade(glyphs),
                font: font.clone(),
                color: *color,
            })
        } else {
            None
        }
    }
    pub fn get(&mut self, key: Option<&Key>) -> Option<Arc<Record>> {
        self.cache.lookup(key?)?.upgrade()
    }
    pub fn store(&mut self, key: Option<Key>, record: &Arc<Record>) {
        let Some(key) = key.filter(|_| self.cache.writing()) else {
            return;
        };
        let mut cost = Cost::new(
            std::mem::size_of::<Key>()
                + std::mem::size_of::<Record>()
                + key.glyphs.as_ptr().len() * std::mem::size_of::<Glyph>(),
        );
        if let Some(resource) = key.font.resource() {
            cost.resource(resource);
        }
        self.cache.store(key, Arc::downgrade(record), cost);
    }
    pub fn maintain(&mut self) {
        self.cache.maintain();
    }
    pub fn clear(&mut self) {
        self.cache.clear();
    }
    pub fn close(&mut self) {
        self.cache.close();
    }
    pub fn publish(&self) {
        self.cache.publish();
    }
}
