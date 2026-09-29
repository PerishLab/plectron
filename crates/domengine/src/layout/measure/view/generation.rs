use crate::layout::{Id, cache::Measurement};
use std::sync::{Arc, Weak};

#[derive(Debug, Clone)]
pub(in crate::layout) struct Generation {
    root: Arc<Measurement>,
    sources: Arc<[Option<Weak<Measurement>>]>,
}

impl Generation {
    pub(in crate::layout) fn new(
        root: Arc<Measurement>,
        sources: Arc<[Option<Weak<Measurement>>]>,
    ) -> Self {
        Self { root, sources }
    }
    pub(in crate::layout) fn root(&self) -> &Arc<Measurement> {
        &self.root
    }
    pub(super) fn source(&self, slot: usize) -> Option<Arc<Measurement>> {
        self.sources.get(slot)?.as_ref()?.upgrade()
    }
    pub(in crate::layout) fn find(
        &self,
        snapshot: &super::super::snapshot::Snapshot,
        id: Id,
    ) -> Option<Arc<Measurement>> {
        self.source(snapshot.slot(id)?)
    }
    pub(in crate::layout) fn sources(&self) -> &Arc<[Option<Weak<Measurement>>]> {
        &self.sources
    }
}
