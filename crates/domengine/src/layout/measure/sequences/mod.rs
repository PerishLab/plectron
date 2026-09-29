mod flow;
use super::Measurement;
use crate::layout::input::{Entry as Node, Key, Record};
use crate::{Constraints, Fault};
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

pub(in crate::layout) const LENGTH: usize = 32;
const POLICY: Policy = Policy {
    read: false,
    write: false,
    entries: 256,
    bytes: 16_777_216,
};
type Identity = (u64, Constraints, usize);
fn identity(element: &Record, constraints: Constraints, gap: usize) -> Identity {
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    element.key.hash(&mut hash);
    (hash.finish(), constraints, gap)
}
#[derive(Clone)]
struct Entry {
    size: [usize; 2],
    overflow: [usize; 2],
    children: Arc<[Reference]>,
    attachments: Arc<[geoengine::Attachment]>,
    work: usize,
}
struct Reference {
    key: std::sync::Weak<Key>,
    offset: [usize; 2],
    geometry: std::sync::Weak<Measurement>,
}
impl Entry {
    fn bind(&self, elements: &[Node<'_>]) -> Option<Arc<Measurement>> {
        if self.children.len() != elements.len() {
            return None;
        }
        let children = self
            .children
            .iter()
            .zip(elements)
            .map(|(child, element)| {
                if child.key.upgrade()?.as_ref() != element.key.as_ref() {
                    return None;
                }
                Some(crate::layout::cache::Placement {
                    offset: child.offset,
                    geometry: child.geometry.upgrade()?,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Arc::new(Measurement {
            work: self.work,
            pending: false,
            size: self.size,
            overflow: self.overflow,
            clipped: false,
            children,
            text: None,
            quality: geoengine::Quality::Measured,
        }))
    }
}
pub(in crate::layout) struct Sequences {
    enabled: bool,
    cache: Cache<Identity, Entry>,
    config: config::Store,
    setting: Option<config::Value>,
}
impl Sequences {
    pub fn enabled(&self) -> bool {
        self.enabled && (self.cache.reading() || self.cache.writing())
    }
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            enabled: true,
            cache: manager.register("sequences", POLICY)?,
            config,
            setting: None,
        })
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        let setting = self.config.get("layout.sequences");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config layout.sequences: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.setting = setting;
        }
        Ok(())
    }
    pub fn enable(&mut self, enabled: bool) {
        self.enabled = enabled;
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
    pub fn publish(&mut self) {
        self.cache.publish();
    }
}
