use crate::layout::measure::snapshot::bindings::Bindings;
use crate::layout::text::paragraph::Lines;
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};

pub(in crate::layout) struct Payloads {
    cache: Cache<geoengine::Attachment, Arc<Lines>>,
    config: config::Store,
    setting: Option<config::Value>,
}

impl Payloads {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("attachments", POLICY)?,
            config,
            setting: None,
        })
    }

    pub fn configure(&mut self) -> Result<(), crate::Fault> {
        let setting = self.config.get("layout.attachments");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| crate::Fault(format!("config layout.attachments: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| crate::Fault(error.to_string()))?;
            self.setting = setting;
        }
        Ok(())
    }

    pub fn store(&mut self, lines: Arc<Lines>) {
        self.cache
            .store(lines.attachment(), lines.clone(), lines.cost().clone());
    }

    pub fn resolve(
        &mut self,
        attachments: &[geoengine::Attachment],
        bindings: &mut Bindings,
    ) -> Result<bool, crate::Fault> {
        let Some(lines) = attachments
            .iter()
            .map(|attachment| self.cache.get(attachment).map(|lines| (*attachment, lines)))
            .collect::<Option<Vec<_>>>()
        else {
            return Ok(false);
        };
        for (attachment, lines) in lines {
            bindings.bind(attachment, lines)?;
        }
        Ok(true)
    }

    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn maintain(&mut self) {
        self.cache.maintain();
    }

    pub fn close(&mut self) {
        self.cache.close();
    }

    pub fn publish(&self) {
        self.cache.publish();
    }

    pub fn enable(&mut self, enabled: bool) {
        let mut policy = self.cache.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.cache.configure(policy);
    }
}
