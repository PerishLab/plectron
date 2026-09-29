use crate::{Fault, Font};
use serde::Deserialize;
use shared::cache::{Cache, Cost, Manager, Policy};

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};

pub(super) struct Selector {
    cache: Cache<(Vec<Font>, String), usize>,
    config: config::Store,
    policy: Option<config::Value>,
}

impl Selector {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("selection", POLICY)?,
            config,
            policy: None,
        })
    }

    pub fn maintain(&mut self) {
        self.cache.maintain();
    }

    pub fn configure(&mut self) -> Result<(), Fault> {
        self.maintain();
        let value = self.config.get("text.selection");
        if value != self.policy {
            let policy = value
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config text.selection: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.policy = value;
        }
        Ok(())
    }

    pub fn choose(&mut self, source: &str, fonts: &[Font]) -> Result<usize, Fault> {
        let key = (fonts.to_vec(), source.to_owned());
        if let Some(index) = self.cache.get(&key) {
            return Ok(index);
        }
        let index = super::fallback::choose(source, fonts)?;
        if self.cache.writing() {
            let mut cost = Cost::new(
                std::mem::size_of_val(fonts)
                    + source.len()
                    + std::mem::size_of_val(&key)
                    + std::mem::size_of::<usize>(),
            );
            for font in fonts {
                if let Some(resource) = font.resource() {
                    cost.resource(resource);
                }
            }
            self.cache.store(key, index, cost);
        }
        Ok(index)
    }

    pub fn publish(&self) {
        self.cache.publish();
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.publish();
    }

    pub fn close(&mut self) {
        self.cache.close();
    }
}
