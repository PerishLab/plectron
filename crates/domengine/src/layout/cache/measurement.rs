use super::super::input::Entry;
use super::super::measure::Measured;
use crate::Node;
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 262_144,
};

pub(in crate::layout) struct Estimates {
    cache: Cache<Node, Estimate>,
    config: config::Store,
    policy: Option<config::Value>,
}

impl Estimates {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("estimates", POLICY)?,
            config,
            policy: None,
        })
    }

    pub fn configure(&mut self) -> Result<(), crate::Fault> {
        let value = self.config.get("layout.estimates");
        if value != self.policy {
            let policy = value
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| crate::Fault(format!("config layout.estimates: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| crate::Fault(error.to_string()))?;
            self.policy = value;
        }
        Ok(())
    }

    pub fn get(&mut self, node: &Node) -> Option<Estimate> {
        self.cache.get(node)
    }

    pub fn insert(&mut self, node: Node, estimate: Estimate) {
        self.cache.insert(
            node,
            estimate,
            std::mem::size_of::<Node>() + std::mem::size_of::<Estimate>(),
        );
    }

    pub fn report(&self, report: &mut super::Estimation) {
        let stats = self.cache.stats();
        report.entries = stats.entries;
        report.bytes = stats.bytes;
        report.evicted = stats.evicted as usize;
        report.omitted = stats.omitted as usize;
        self.cache.publish();
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
}

#[derive(Debug, Clone, Copy)]
pub(in crate::layout) struct Estimate {
    pub fingerprint: u64,
    pub width: usize,
    pub height: usize,
    pub quality: geoengine::Quality,
}

impl Estimate {
    pub fn quality(
        values: Option<&mut Estimates>,
        element: Entry<'_>,
        width: usize,
    ) -> geoengine::Quality {
        match values.and_then(|values| values.get(&element.node)) {
            Some(estimate) if estimate.matches(element, width) => estimate.quality,
            Some(_) => geoengine::Quality::Stale,
            None if element.key.style.intrinsic.is_some() && element.key.style.height.is_none() => {
                geoengine::Quality::Estimated
            }
            None => geoengine::Quality::Measured,
        }
    }

    pub fn matches(&self, element: Entry<'_>, width: usize) -> bool {
        self.fingerprint == element.key.fingerprint() && self.width == width
    }

    pub fn unresolved(&self, fingerprint: u64, width: usize) -> bool {
        self.quality != geoengine::Quality::Measured
            || self.fingerprint != fingerprint
            || self.width != width
    }
}

#[derive(Debug, Clone)]
pub(in crate::layout) struct Measurement {
    pub work: usize,
    pub pending: bool,
    pub size: [usize; 2],
    pub overflow: [usize; 2],
    pub clipped: bool,
    pub children: Vec<Placement>,
    pub text: Option<geoengine::Attachment>,
    pub quality: geoengine::Quality,
}
#[derive(Debug, Clone)]
pub(in crate::layout) struct Placement {
    pub offset: [usize; 2],
    pub geometry: Arc<Measurement>,
}
impl Measurement {
    pub fn accepts(
        &self,
        element: Entry<'_>,
        demand: Option<&std::collections::BTreeSet<crate::Node>>,
        mut estimates: Option<&mut super::Estimates>,
    ) -> bool {
        if !element.key.isolated {
            return true;
        }
        if element.key.style.independent() {
            let active = demand.is_none_or(|nodes| nodes.contains(&element.node));
            if self.pending == active {
                return false;
            }
            if self.pending {
                let quality =
                    super::Estimate::quality(estimates.as_deref_mut(), element, self.size[0]);
                return self.quality == quality;
            }
        }
        self.children
            .iter()
            .zip(element.children())
            .all(|(placement, child)| {
                placement
                    .geometry
                    .accepts(child, demand, estimates.as_deref_mut())
            })
    }
    pub fn cost(&self) -> shared::cache::Cost {
        let mut cost = shared::cache::Cost::new(std::mem::size_of::<Self>());
        cost.bytes += std::mem::size_of_val(self.children.as_slice());
        for child in &self.children {
            cost.extend(&child.geometry.cost());
        }
        cost
    }
    pub fn attachments(&self) -> Arc<[geoengine::Attachment]> {
        let mut attachments = std::collections::BTreeSet::new();
        self.collect(&mut attachments);
        attachments.into_iter().collect::<Vec<_>>().into()
    }
    pub fn shaped(&self) -> bool {
        self.text.is_some()
    }
    fn collect(&self, attachments: &mut std::collections::BTreeSet<geoengine::Attachment>) {
        if let Some(attachment) = self.text {
            attachments.insert(attachment);
        }
        for child in &self.children {
            child.geometry.collect(attachments);
        }
    }
    pub fn bind<'a>(self: &Arc<Self>, element: Entry<'a>) -> Measured<'a> {
        Measured {
            element,
            offset: [0; 2],
            geometry: self.clone(),
        }
    }
}
impl<'a> Measured<'a> {
    pub fn new(element: Entry<'a>, geometry: Measurement) -> Self {
        Self {
            element,
            offset: [0; 2],
            geometry: Arc::new(geometry),
        }
    }
    pub fn children(&self) -> impl Iterator<Item = Measured<'a>> + '_ {
        self.geometry
            .children
            .iter()
            .zip(self.element.children())
            .map(|(placement, element)| Measured {
                element,
                offset: placement.offset,
                geometry: placement.geometry.clone(),
            })
    }
    pub fn coverage(&self, axis: usize) -> usize {
        if self.clipped {
            return self.outer(axis);
        }
        self.outer(axis)
            .max(self.overflow[axis] + self.element.key.style.inset())
    }
    pub fn outer(&self, axis: usize) -> usize {
        self.size[axis] + 2 * (self.element.key.style.padding + self.element.key.style.margin)
    }
    pub fn placement(self) -> Placement {
        Placement {
            offset: self.offset,
            geometry: self.geometry,
        }
    }
}
impl std::ops::Deref for Measured<'_> {
    type Target = Measurement;
    fn deref(&self) -> &Measurement {
        &self.geometry
    }
}
