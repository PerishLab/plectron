mod basis;
mod collection;
mod generation;
use super::Material;
use super::snapshot::Snapshot;
use crate::layout::text::paragraph::Lines;
pub(in crate::layout) use basis::{Basis, Source};
pub(in crate::layout) use collection::{Collected, Context, collect};
pub(in crate::layout) use generation::Generation;
pub use geoengine::{Metric, State};
use std::sync::Arc;
#[derive(Clone)]
pub(in crate::layout) struct View<'a> {
    snapshot: &'a Arc<Snapshot>,
    entry: geoengine::Entry<'a>,
    pub offset: [usize; 2],
    metric: &'a Metric,
    pub text: Option<&'a Arc<Lines>>,
}
impl<'a> View<'a> {
    pub(super) fn new(snapshot: &'a Arc<Snapshot>, entry: geoengine::Entry<'a>) -> Option<Self> {
        let metric = entry.metric()?;
        let text = match metric.attachment() {
            Some(attachment) => Some(snapshot.binding(attachment)?),
            None => None,
        };
        Some(Self {
            snapshot,
            entry,
            offset: metric.origin().array().expect("DOM geometry origin"),
            metric,
            text,
        })
    }
    pub fn children(&self) -> impl Iterator<Item = View<'a>> + '_ {
        self.entry
            .children()
            .filter_map(|entry| Self::new(self.snapshot, entry))
    }
    pub fn relative(&self, offset: [usize; 2]) -> Self {
        Self {
            offset,
            ..self.clone()
        }
    }
    pub fn deferred(&self) -> bool {
        matches!(self.entry.state(), State::Deferred(_))
    }
    pub fn material(&self) -> &Material {
        self.snapshot.material(self.entry.slot())
    }
    pub fn size(&self) -> [usize; 2] {
        self.metric.size().array().expect("DOM geometry size")
    }
    pub fn overflow(&self) -> [usize; 2] {
        self.metric
            .overflow()
            .array()
            .expect("DOM geometry overflow")
    }
    pub fn clipped(&self) -> bool {
        self.metric.clipped()
    }
    pub fn basis(&self) -> Basis {
        Basis::new(
            self.metric.id,
            self.entry.stamp(),
            self.entry
                .signature()
                .expect("resolved geometry signature")
                .clone(),
            self.children().map(|child| child.text.cloned()).collect(),
        )
    }
    pub fn matches(&self, basis: &Basis) -> bool {
        basis.matches(self.entry.stamp())
    }
}
impl std::ops::Deref for View<'_> {
    type Target = Metric;

    fn deref(&self) -> &Self::Target {
        self.metric
    }
}
