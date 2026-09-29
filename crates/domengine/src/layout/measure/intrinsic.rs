use super::Measure;
use crate::layout::input::{Entry, Key};
use crate::{Available, Constraints, Fault, Shaper};
use serde::{Deserialize, Serialize};
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};
type Identity = (Arc<Key>, Constraints);

#[derive(Clone, Copy)]
struct Sample {
    size: [usize; 2],
    work: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Report {
    pub requests: usize,
    pub hits: usize,
    pub budgets: usize,
}

pub(in crate::layout) struct Intrinsic {
    cache: Cache<Identity, Sample>,
    config: config::Store,
    policy: Option<config::Value>,
    pub report: Report,
}

impl Intrinsic {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("intrinsic", POLICY)?,
            config,
            policy: None,
            report: Report::default(),
        })
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        self.report = Report::default();
        let value = self.config.get("layout.intrinsic");
        if value != self.policy {
            let policy = value
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config layout.intrinsic: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.policy = value;
        }
        Ok(())
    }
    fn find(&mut self, key: &Identity, remaining: &mut usize) -> Option<[usize; 2]> {
        self.report.requests += 1;
        let sample = self.cache.get(key)?;
        if *remaining < sample.work - 1 {
            self.report.budgets += 1;
            return None;
        }
        *remaining -= sample.work - 1;
        self.report.hits += 1;
        Some(sample.size)
    }
    fn store(&mut self, key: Identity, sample: Sample) {
        let bytes = key.0.weight + std::mem::size_of::<Identity>() + std::mem::size_of::<Sample>();
        self.cache.insert(key, sample, bytes);
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
    pub fn enable(&mut self, enabled: bool) {
        let mut policy = self.cache.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.cache.configure(policy);
    }
}

impl<S: Shaper> Measure<'_, S> {
    pub(in crate::layout) fn minimum(
        &mut self,
        element: Entry<'_>,
        axis: usize,
        mut constraints: Constraints,
    ) -> Result<usize, Fault> {
        self.operations.minimums += 1;
        if element.key.style.independent() {
            return Ok(element
                .key
                .style
                .bound(axis, element.key.style.preferred(axis).unwrap_or(0)));
        }
        constraints.known[axis] = None;
        constraints.available[axis] = Available::Min;
        if element.key.text.is_some() || element.key.image.is_some() {
            return Ok(self.node(element, constraints)?.size[axis]);
        }
        let before = self.remaining;
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| Fault("layout work budget exhausted".into()))?;
        let key = (element.key.clone(), constraints);
        if let Some(intrinsic) = &mut self.intrinsic
            && let Some(size) = intrinsic.find(&key, &mut self.remaining)
        {
            return Ok(size[axis]);
        }
        let size = self.boxes(element, constraints)?.size;
        if let Some(intrinsic) = &mut self.intrinsic {
            intrinsic.store(
                key,
                Sample {
                    size,
                    work: before - self.remaining,
                },
            );
        }
        Ok(size[axis])
    }
}
