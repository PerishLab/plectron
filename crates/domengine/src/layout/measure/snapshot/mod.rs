use super::{Generation, Impact, Metric, State, impact::Prediction, view};
use crate::{
    Fault, Node,
    layout::{Constraints, Id, Tree, cache::Measurement, delta},
};
use std::sync::Arc;

mod material;
pub(in crate::layout) use material::{History, Material};
pub(in crate::layout) mod bindings;
mod residency;
pub use residency::{Lifetime, Liveness, Memory, References, Residency};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Collection {
    pub visited: usize,
    pub reused: usize,
    pub pages: usize,
    pub shared: usize,
    pub materialized: usize,
    pub copied: usize,
}
type Evaluation<'a> = (
    Constraints,
    Option<&'a [Node]>,
    Option<(&'a Snapshot, &'a Generation)>,
    Option<Prediction>,
    bool,
    Option<usize>,
);

#[derive(Clone)]
pub struct Snapshot {
    tree: Arc<Tree>,
    layout: Arc<geoengine::Layout>,
    bindings: Arc<bindings::Bindings>,
    materials: Arc<[Option<Material>]>,
    impact: Option<Impact>,
    constraints: Constraints,
    demand: Option<Arc<[Node]>>,
    collection: Collection,
}

impl std::fmt::Debug for Snapshot {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Snapshot")
            .field("generation", &self.layout.generation())
            .field("tree", &self.tree)
            .field("states", &self.layout.states())
            .field("delta", &self.layout.delta())
            .field("impact", &self.impact)
            .field("constraints", &self.constraints)
            .field("demand", &self.demand)
            .field("collection", &self.collection)
            .finish()
    }
}

impl Snapshot {
    pub(in crate::layout) fn build(
        tree: Arc<Tree>,
        geometry: &Arc<Measurement>,
        current: &bindings::Bindings,
        evaluation: Evaluation<'_>,
    ) -> Result<(Arc<Self>, Generation), Fault> {
        let (constraints, demand, previous, prediction, audit, pages) = evaluation;
        let collectionscope = (!audit)
            .then(|| prediction.as_ref().and_then(Prediction::collection))
            .flatten();
        let retained = previous.filter(|(previous, generation)| {
            Arc::ptr_eq(tree.element(), previous.tree.element())
                && Arc::ptr_eq(geometry, generation.root())
        });
        let (states, sources, bindings, collection) = if let Some(previous) = retained {
            let (previous, generation) = previous;
            (
                previous.layout.states().clone(),
                generation.sources().clone(),
                previous.bindings.clone(),
                Collection {
                    visited: 0,
                    reused: previous.layout.states().count(),
                    pages: previous.layout.states().pages(),
                    shared: previous.layout.states().pages(),
                    materialized: 0,
                    copied: 0,
                },
            )
        } else {
            view::collect(&tree, geometry, (current, previous, collectionscope, pages))?
        };
        let scope = (!audit)
            .then(|| prediction.as_ref().and_then(Prediction::scope))
            .flatten();
        let layout = geoengine::Layout::new(
            tree.facts().clone(),
            states,
            previous.map(|(snapshot, _)| snapshot.layout.as_ref()),
            scope,
        )
        .map_err(|error| Fault(error.to_string()))?;
        let impact = audit
            .then(|| prediction.map(|prediction| Impact::audit(prediction, layout.delta())))
            .flatten();
        let materials = layout
            .states()
            .iter()
            .enumerate()
            .map(|(slot, state)| {
                state
                    .metric()
                    .map(|_| Material::new(tree.entry(slot).record().clone()))
            })
            .collect::<Vec<_>>()
            .into();
        Ok((
            Arc::new(Self {
                tree,
                layout,
                bindings,
                materials,
                impact,
                constraints,
                demand: demand.map(Arc::from),
                collection,
            }),
            Generation::new(geometry.clone(), sources),
        ))
    }
    pub fn generation(&self) -> u64 {
        self.layout.generation()
    }
    pub fn tree(&self) -> &Arc<Tree> {
        &self.tree
    }
    pub fn states(&self) -> &geoengine::Store<State> {
        self.layout.states()
    }
    pub fn collection(&self) -> Collection {
        self.collection
    }
    pub fn publication(&self) -> geoengine::Publication {
        self.layout.publication()
    }
    pub fn timing(&self) -> geoengine::Timing {
        self.layout.timing()
    }
    pub fn shape(&self) -> geoengine::Shape {
        self.layout.shape()
    }
    pub fn residency(&self) -> geoengine::Residency {
        self.layout.residency()
    }
    pub fn metrics(&self) -> impl Iterator<Item = &Metric> {
        self.layout.states().iter().filter_map(State::metric)
    }
    pub fn metric(&self, id: Id) -> Option<&Metric> {
        self.layout.metric(id)
    }
    pub fn delta(&self) -> &delta::Layout {
        self.layout.delta()
    }
    pub fn impact(&self) -> Option<&Impact> {
        self.impact.as_ref()
    }
    pub fn constraints(&self) -> Constraints {
        self.constraints
    }
    pub fn demand(&self) -> Option<&[Node]> {
        self.demand.as_deref()
    }
    pub(in crate::layout) fn view(self: &Arc<Self>) -> super::View<'_> {
        super::View::new(self, self.layout.root()).expect("layout root is available")
    }

    pub(in crate::layout) fn material(&self, slot: usize) -> &Material {
        self.materials[slot]
            .as_ref()
            .expect("resolved layout material")
    }

    pub(in crate::layout) fn binding(
        &self,
        attachment: geoengine::Attachment,
    ) -> Option<&Arc<super::super::text::paragraph::Lines>> {
        self.bindings.get(attachment)
    }

    pub(in crate::layout) fn slot(&self, id: Id) -> Option<usize> {
        self.layout.entry(id).map(geoengine::Entry::slot)
    }

    pub(in crate::layout) fn pin(
        &self,
        geometry: &Arc<Measurement>,
        target: &mut bindings::Bindings,
    ) -> Result<(), Fault> {
        if target.pin(geometry, &self.bindings)? {
            Ok(())
        } else {
            Err(Fault("published geometry attachment is absent".into()))
        }
    }
}
