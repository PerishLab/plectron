use super::Program;
use crate::{Face, Fault, Settings};
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 256,
    bytes: 33_554_432,
};

#[derive(PartialEq, Eq, Hash)]
struct Key {
    font: u64,
    script: [u8; 4],
    language: Option<[u8; 4]>,
    feature: Option<[u8; 4]>,
    options: (bool, bool),
}
pub(crate) struct Compiler {
    cache: Cache<Key, Arc<Program>>,
    config: config::Store,
    policy: Option<config::Value>,
}
impl Compiler {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("shaping", POLICY)?,
            config,
            policy: None,
        })
    }
    pub fn maintain(&mut self) {
        self.cache.maintain();
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        self.maintain();
        let value = self.config.get("text.shaping");
        if value != self.policy {
            let policy = value
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config text.shaping: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.policy = value;
        }
        Ok(())
    }
    pub(in crate::shape) fn get(
        &mut self,
        face: &Face,
        settings: Settings,
    ) -> Result<Option<Arc<Program>>, Fault> {
        let Face::Owned(owner) = face else {
            return Ok(None);
        };
        if !self.cache.reading() && !self.cache.writing() {
            return Ok(None);
        }
        let key = Key {
            font: owner.id(),
            script: settings.script,
            language: settings.language,
            feature: settings.feature,
            options: (settings.ligatures, settings.marks),
        };
        if let Some(program) = self.cache.get(&key) {
            return Ok(Some(program));
        }
        let Ok(program) = Program::new(owner.clone(), settings) else {
            return Ok(None);
        };
        let program = Arc::new(program);
        let mut cost = program.cost();
        cost.bytes += std::mem::size_of::<Key>() + std::mem::size_of::<Arc<Program>>();
        self.cache.store(key, program.clone(), cost);
        Ok(Some(program))
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
