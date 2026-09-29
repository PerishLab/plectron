use crate::Fault;
use serde::Deserialize;
use shared::cache::Policy;
use std::hash::{Hash, Hasher};

pub(super) const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};
const WORK: usize = 64;
const RECURRENCE: usize = 4;
const SEEN: usize = 4096;

pub(super) struct Admission {
    config: config::Store,
    policy: Option<config::Value>,
    maximum: usize,
    recurrence: usize,
    seen: std::collections::HashSet<u64>,
    pub dependencies: bool,
}
impl Admission {
    pub fn new(config: config::Store) -> Self {
        Self {
            config,
            policy: None,
            maximum: WORK,
            recurrence: RECURRENCE,
            seen: Default::default(),
            dependencies: true,
        }
    }
    pub fn accepts(&self, work: usize) -> bool {
        work <= self.maximum
    }
    pub fn transient(&self) -> bool {
        self.maximum > 0
    }
    pub fn recurring(&mut self, key: &impl Hash, work: usize) -> bool {
        if self.recurrence == 0 || work > self.recurrence {
            return true;
        }
        let mut hash = ahash::AHasher::default();
        key.hash(&mut hash);
        let hash = hash.finish();
        if self.seen.contains(&hash) {
            return true;
        }
        if self.seen.len() == SEEN {
            self.seen.clear();
        }
        self.seen.insert(hash);
        false
    }
    pub fn clear(&mut self) {
        self.seen.clear();
    }
    pub fn update(&mut self) -> Result<Option<Policy>, Fault> {
        self.dependencies = self
            .config
            .get("layout.dependencies")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.dependencies: {error}")))?
            .unwrap_or(true);
        let maximum = self
            .config
            .get("layout.admission")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.admission: {error}")))?
            .unwrap_or(WORK);
        let recurrence = self
            .config
            .get("layout.recurrence")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.recurrence: {error}")))?
            .unwrap_or(RECURRENCE);
        if recurrence != self.recurrence {
            self.seen.clear();
        }
        self.recurrence = recurrence;
        let value = self.config.get("layout.cache");
        let policy = if value != self.policy {
            Some(
                value
                    .clone()
                    .map(Policy::deserialize)
                    .transpose()
                    .map_err(|error| Fault(format!("config layout.cache: {error}")))?
                    .unwrap_or(POLICY),
            )
        } else {
            None
        };
        self.policy = value;
        self.maximum = maximum;
        Ok(policy)
    }
}
