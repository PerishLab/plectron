use super::{envelope::Envelope, outlines::Outlines};
use crate::shared::{Fault, Ink, Output, Record};
use serde::Deserialize;
use shared::cache::{Cache, Command, Manager, Policy, Snapshot};
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 2048,
    bytes: 16_777_216,
};

struct Key {
    record: Arc<Record>,
    scale: u64,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.scale == other.scale && Arc::ptr_eq(&self.record, &other.record)
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.record).hash(state);
        self.scale.hash(state);
    }
}
pub(super) struct Visibility {
    config: config::Store,
    setting: Option<config::Value>,
    manager: Manager,
    cache: Cache<Key, Envelope>,
}
impl Visibility {
    pub fn new(config: config::Store) -> Self {
        let manager = Manager::default();
        let cache = manager
            .register("bounds", POLICY)
            .expect("fresh bounds cache");
        Self {
            config,
            setting: None,
            manager,
            cache,
        }
    }
    pub fn maintain(&mut self) -> Result<(), Fault> {
        let setting = self.config.get("render.bounds");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config render.bounds: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|e| Fault(e.to_string()))?;
            self.setting = setting;
        }
        self.cache.maintain();
        Ok(())
    }
    pub fn inspect(&self) -> Snapshot {
        self.cache.publish();
        self.manager.inspect().remove(0)
    }
    pub fn command(&mut self, command: Command) -> Result<(), Fault> {
        self.manager
            .command(self.cache.id(), command)
            .map_err(|e| Fault(e.to_string()))?;
        self.cache.maintain();
        Ok(())
    }
    pub fn get(
        &mut self,
        record: &Arc<Record>,
        output: Output,
        context: (&mut Outlines, &mut super::Work),
    ) -> Result<Option<Envelope>, Fault> {
        if !self.cache.reading() && !self.cache.writing() {
            return Ok(None);
        }
        let key = Key {
            record: record.clone(),
            scale: output.scale().to_bits(),
        };
        if let Some(bounds) = self.cache.get(&key) {
            return Ok(Some(bounds));
        }
        let Ink::Text { font, glyphs, .. } = record.ink() else {
            return Ok(None);
        };
        if !super::envelope::bounded(glyphs) {
            return Ok(None);
        }
        context.1.measured += 1;
        let mut bounds = Envelope::new();
        for glyph in glyphs.iter() {
            let (path, decoded) = context.0.get(font, glyph.id)?;
            context.1.outlines += 1;
            context.1.decoded += u64::from(decoded);
            let transform = output.transform(font.transform().translate([glyph.x, glyph.y]));
            let Ok(hull) = super::placement::bounds(&path, transform) else {
                return Ok(None);
            };
            bounds.include(hull);
        }
        let mut cost = record.ink().cost();
        cost.bytes += std::mem::size_of::<Record>()
            + std::mem::size_of::<Key>()
            + std::mem::size_of::<Envelope>();
        self.cache.store(key, bounds, cost);
        Ok(Some(bounds))
    }
}
