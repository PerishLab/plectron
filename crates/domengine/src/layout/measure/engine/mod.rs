use super::{Prior, intrinsic, snapshot};
use crate::layout::{cache, input::Entry};
use std::sync::Arc;

mod block;
mod record;

pub(in crate::layout) struct Measured<'a> {
    pub element: Entry<'a>,
    pub offset: [usize; 2],
    pub geometry: Arc<cache::Measurement>,
}

pub(in crate::layout) struct Measure<'a, S> {
    pub demand: Option<&'a std::collections::BTreeSet<crate::Node>>,
    pub trace: cache::Trace,
    pub shaper: &'a S,
    pub memo: Option<&'a mut cache::Memo>,
    pub intrinsic: Option<&'a mut intrinsic::Intrinsic>,
    pub prior: Option<Prior>,
    pub estimates: Option<&'a mut cache::Estimates>,
    pub bindings: snapshot::bindings::Bindings,
    pub operations: cache::Operations,
    pub causality: Option<&'a mut cache::Causality>,
    pub remaining: usize,
    pub items: usize,
}
