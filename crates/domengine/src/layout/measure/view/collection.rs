use super::super::snapshot::{Collection, Snapshot};
use super::{Generation, Metric, State};
use crate::{
    Fault,
    layout::{Id, Tree, cache::Measurement},
};
use std::sync::{Arc, Weak};

type Collected = (
    geoengine::Store<State>,
    Arc<[Option<Weak<Measurement>>]>,
    Arc<super::super::snapshot::bindings::Bindings>,
    Collection,
);
type Context<'a> = (
    &'a super::super::snapshot::bindings::Bindings,
    Option<(&'a Snapshot, &'a Generation)>,
    Option<&'a [Id]>,
    Option<usize>,
);

pub(in crate::layout) fn collect(
    tree: &Tree,
    geometry: &Arc<Measurement>,
    history: Context<'_>,
) -> Result<Collected, Fault> {
    let (current, previous, scope, pages) = history;
    let retained = previous
        .filter(|_| scope.is_some())
        .map(|(previous, _)| previous.states());
    let mut collector = Collector {
        tree,
        current,
        previous,
        scope,
        states: match pages {
            Some(width) => geoengine::Store::paged(tree.nodes().len(), retained, width),
            None => geoengine::Store::build(tree.nodes().len(), retained),
        }
        .map_err(|error| Fault(error.to_string()))?,
        sources: vec![None; tree.nodes().len()],
        bindings: Default::default(),
        collection: Collection::default(),
    };
    let root = tree.slot(tree.root()).expect("layout root");
    collector.resolve((root, [0; 2]), geometry)?;
    let mut collection = collector.collection;
    let traffic = collector.states.traffic();
    let states = collector
        .states
        .finish()
        .map_err(|error| Fault(error.to_string()))?;
    collection.pages = states.pages();
    collection.shared = previous.map_or(0, |(previous, _)| states.shared(previous.states()));
    collection.materialized = traffic.materialized;
    collection.copied = traffic.copied;
    Ok((
        states,
        collector.sources.into(),
        Arc::new(collector.bindings),
        collection,
    ))
}
struct Collector<'a> {
    tree: &'a Tree,
    current: &'a super::super::snapshot::bindings::Bindings,
    previous: Option<(&'a Snapshot, &'a Generation)>,
    scope: Option<&'a [Id]>,
    states: geoengine::Builder<State>,
    sources: Vec<Option<Weak<Measurement>>>,
    bindings: super::super::snapshot::bindings::Bindings,
    collection: Collection,
}
impl Collector<'_> {
    fn resolve(
        &mut self,
        entity: (usize, [usize; 2]),
        geometry: &Arc<Measurement>,
    ) -> Result<(), Fault> {
        let (slot, offset) = entity;
        if self.states.marked(slot) {
            return Err(Fault("layout snapshot contains duplicate geometry".into()));
        }
        if self.reusable(slot, offset, geometry) {
            return self.reuse(slot);
        }
        self.collection.visited += 1;
        let origin =
            geoengine::Point::positive(offset).map_err(|error| Fault(error.to_string()))?;
        let size =
            geoengine::Size::positive(geometry.size).map_err(|error| Fault(error.to_string()))?;
        let overflow = geoengine::Size::positive(geometry.overflow)
            .map_err(|error| Fault(error.to_string()))?;
        let bounds = geoengine::Bounds::new(size, overflow, geometry.clipped);
        let attachment = geometry.text;
        let metric = Metric::new(
            self.tree.nodes()[slot].id,
            origin,
            (bounds, attachment, geometry.quality),
        );
        self.sources[slot] = Some(Arc::downgrade(geometry));
        if let Some(attachment) = geometry.text {
            let pinned = self
                .current
                .get(attachment)
                .ok_or_else(|| Fault("geometry attachment is not pinned".into()))?;
            self.bindings.bind(attachment, pinned.clone())?;
        }
        let children = self.tree.children(slot).collect::<Vec<_>>();
        if geometry.pending {
            if !geometry.children.is_empty() {
                return Err(Fault("deferred layout contains resolved children".into()));
            }
            let boundary = metric.id;
            self.write(slot, State::Deferred(metric))?;
            for child in children {
                self.block(child, boundary)?;
            }
            return Ok(());
        }
        if children.len() != geometry.children.len() {
            return Err(Fault(format!(
                "layout geometry does not match tree topology at {:?}: tree={} geometry={}",
                metric.id,
                children.len(),
                geometry.children.len()
            )));
        }
        self.write(slot, State::Resolved(metric))?;
        for (child, placement) in children.into_iter().zip(&geometry.children) {
            self.resolve((child, placement.offset), &placement.geometry)?;
        }
        Ok(())
    }

    fn reusable(&self, slot: usize, offset: [usize; 2], geometry: &Arc<Measurement>) -> bool {
        let Some((previous, generation)) = self.previous else {
            return false;
        };
        let Some(scope) = self.scope else {
            return false;
        };
        let id = self.tree.nodes()[slot].id;
        if scope.binary_search(&id).is_ok() {
            return false;
        }
        if previous.slot(id) != Some(slot) {
            return false;
        }
        previous.states()[slot].metric().is_some_and(|metric| {
            metric.origin().array() == Some(offset)
                && generation
                    .source(slot)
                    .is_some_and(|source| Arc::ptr_eq(&source, geometry))
        })
    }

    fn write(&mut self, slot: usize, state: State) -> Result<(), Fault> {
        self.states
            .write(slot, state)
            .map_err(|error| Fault(error.to_string()))
    }

    fn block(&mut self, slot: usize, boundary: Id) -> Result<(), Fault> {
        self.states
            .write(slot, State::Blocked(boundary))
            .map_err(|error| Fault(error.to_string()))?;
        self.collection.visited += 1;
        let children = self.tree.children(slot).collect::<Vec<_>>();
        for child in children {
            self.block(child, boundary)?;
        }
        Ok(())
    }

    fn reuse(&mut self, slot: usize) -> Result<(), Fault> {
        self.states
            .retain(slot)
            .map_err(|error| Fault(error.to_string()))?;
        let (previous, generation) = self.previous.expect("reused snapshot");
        if previous.states()[slot].metric().is_some() {
            let source = generation.source(slot).expect("reused measurement source");
            self.sources[slot] = Some(Arc::downgrade(&source));
            if let Some(attachment) = previous.states()[slot]
                .metric()
                .and_then(Metric::attachment)
            {
                let lines = previous
                    .binding(attachment)
                    .expect("published geometry attachment");
                self.bindings.bind(attachment, lines.clone())?;
            }
        }
        self.collection.reused += 1;
        let children = self.tree.children(slot).collect::<Vec<_>>();
        for child in children {
            self.reuse(child)?;
        }
        Ok(())
    }
}
