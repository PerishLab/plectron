use super::{Painting, source::Source};
use crate::{Content, Fault, Layout, Local, layout::Bridge};
use graphics::shared::{Ink, Item, Record};
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 1024,
    bytes: 16_777_216,
};

pub struct Painter {
    identity: super::identity::Identity,
    cache: Cache<Ink, Arc<Record>>,
    pub(super) report: Painting,
    pub(super) retained: Cache<crate::Node, super::recording::Entry>,
    config: config::Store,
    setting: Option<config::Value>,
    retention: Option<config::Value>,
    pub(super) routing: bool,
}
impl Default for Painter {
    fn default() -> Self {
        Self::managed(&Manager::default()).expect("fresh paint cache manager")
    }
}
impl Painter {
    pub fn managed(manager: &Manager) -> Result<Self, shared::cache::Fault> {
        Self::configured(manager, config::Store::default())
    }
    pub fn configured(
        manager: &Manager,
        config: config::Store,
    ) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            identity: super::identity::Identity::new(manager, config.clone())?,
            cache: manager.register("paint", POLICY)?,
            report: Painting::default(),
            retained: manager.register("subsequences", super::recording::POLICY)?,
            config,
            setting: None,
            retention: None,
            routing: true,
        })
    }
    pub fn maintain(&mut self) {
        self.identity.maintain();
        self.cache.maintain();
        self.retained.maintain();
        if !self.cache.reading() {
            self.identity.clear();
            self.retained.clear();
        }
    }
    pub fn close(&mut self) {
        self.identity.close();
        self.cache.close();
        self.retained.close();
    }
    pub fn inspect(&self) -> Painting {
        self.report.clone()
    }
    pub fn enable(&mut self, enabled: bool) {
        let mut policy = self.cache.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.cache.configure(policy);
    }
    pub fn build(&mut self, layout: &Layout) -> Result<Vec<Item>, Fault> {
        layout.bridge(Bridge::Paint);
        let start = std::time::Instant::now();
        self.maintain();
        self.report = Painting {
            revision: layout.revision(),
            enabled: self.cache.reading(),
            ..Painting::default()
        };
        self.report.groups = layout.root.groups;
        let result = self.configure().and_then(|()| self.compose(layout));
        self.report.failure = result.as_ref().err().map(ToString::to_string);
        self.cache.publish();
        self.identity.publish();
        self.retained.publish();
        self.report.elapsed = start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        result
    }
    fn configure(&mut self) -> Result<(), Fault> {
        self.identity.configure()?;
        self.routing = self
            .config
            .get("paint.routing")
            .map(bool::deserialize)
            .transpose()
            .map_err(|e| Fault(format!("config paint.routing: {e}")))?
            .unwrap_or(true);
        let setting = self.config.get("paint.cache");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config paint.cache: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.setting = setting;
        }
        let retention = self.config.get("paint.fragments");
        if retention != self.retention {
            let policy = retention
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|e| Fault(format!("config paint.fragments: {e}")))?
                .unwrap_or(super::recording::POLICY);
            self.retained
                .configure(policy)
                .map_err(|e| Fault(e.to_string()))?;
            self.retention = retention;
        }
        self.report.enabled = self.cache.reading();
        Ok(())
    }
    pub(super) fn draw(&mut self, fragment: &Local) -> Result<Item, Fault> {
        let source = Source::project(fragment);
        let origin = source.origin;
        self.report.requests += 1;
        let key = self
            .cache
            .reading()
            .then(|| self.identity.key(&source.ink))
            .flatten();
        if let Some(record) = self.identity.get(key.as_ref()) {
            self.report.identified += 1;
            self.report.hits += 1;
            return Ok(Self::place(fragment, origin, record));
        }
        let record = match self.cache.get(&source.ink) {
            Some(record) => {
                self.report.hits += 1;
                record
            }
            None => {
                let ink = Arc::new(source.ink);
                let record =
                    Arc::new(Record::new(ink.clone()).map_err(|fault| Fault(fault.to_string()))?);
                self.report.recorded += 1;
                self.report.glyphs += record.cost() - 1;
                let mut cost = ink.cost();
                cost.bytes += std::mem::size_of::<Record>();
                self.cache.retain(ink, record.clone(), cost);
                record
            }
        };
        self.identity.store(key, &record);
        Ok(Self::place(fragment, origin, record))
    }
    fn place(fragment: &Local, origin: [usize; 2], record: Arc<Record>) -> Item {
        Item::Draw {
            origin,
            clip: match &fragment.content {
                Content::Text { clip, .. } => *clip,
                _ => None,
            },
            record,
        }
    }
}
