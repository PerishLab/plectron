use crate::layout::{Id, text::paragraph::Lines};
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::layout) struct Source {
    stamp: geoengine::Stamp,
    signature: geoengine::Signature,
}

#[derive(Clone)]
pub(in crate::layout) struct Basis {
    pub id: Id,
    pub pending: bool,
    stamp: geoengine::Stamp,
    signature: geoengine::Signature,
    texts: Arc<[Option<Arc<Lines>>]>,
}

impl Basis {
    pub(super) fn new(
        id: Id,
        stamp: geoengine::Stamp,
        signature: geoengine::Signature,
        texts: Arc<[Option<Arc<Lines>>]>,
    ) -> Self {
        Self {
            id,
            pending: signature.deferred(),
            stamp,
            signature,
            texts,
        }
    }
    pub(in crate::layout) fn history(&self) -> Source {
        Source {
            stamp: self.stamp,
            signature: self.signature.clone(),
        }
    }
    pub(in crate::layout) fn size(&self) -> [usize; 2] {
        self.signature.size().array().expect("DOM geometry size")
    }
    pub(in crate::layout) fn quality(&self) -> geoengine::Quality {
        self.signature.quality()
    }
    pub(in crate::layout) fn texts(&self) -> &[Option<Arc<Lines>>] {
        &self.texts
    }
    pub(super) fn matches(&self, stamp: geoengine::Stamp) -> bool {
        self.stamp == stamp
    }
}

impl Source {
    pub(in crate::layout) fn matches(&self, current: &Basis) -> bool {
        self.stamp == current.stamp
    }
    pub(in crate::layout) fn equivalent(&self, current: &Basis) -> Option<bool> {
        Some(self.signature == current.signature)
    }
}
