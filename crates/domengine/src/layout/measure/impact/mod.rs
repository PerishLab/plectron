use crate::{
    Node,
    layout::{Constraints, Id, Tree, delta},
};
use std::sync::Arc;

pub use geoengine::Survey;
pub(in crate::layout) type Prediction = geoengine::Plan;

#[derive(Debug, Clone)]
pub struct Impact(geoengine::Impact);

impl Impact {
    pub fn seeds(&self) -> &[Id] {
        self.0.seeds()
    }

    pub fn affected(&self) -> &[Id] {
        self.0.affected()
    }

    pub fn missed(&self) -> &[Id] {
        self.0.missed()
    }

    pub fn excess(&self) -> &[Id] {
        self.0.excess()
    }

    pub fn survey(&self) -> Survey {
        self.0.survey()
    }

    pub(in crate::layout) fn admit(
        tree: &Tree,
        constraints: Constraints,
        prior: Constraints,
    ) -> bool {
        const MINIMUM: usize = 128;
        constraints == prior
            && tree.nodes().len() >= MINIMUM
            && tree.plan().frontier().len() <= tree.nodes().len() / 4
    }

    pub(in crate::layout) fn predict(
        tree: &Tree,
        evaluation: (Constraints, Option<&[Node]>),
        previous: Option<(&Tree, Constraints, Option<&[Node]>)>,
    ) -> Prediction {
        let (constraints, demand) = evaluation;
        let (previous, limits, prior) = previous.map_or((None, None, None), |value| {
            (Some(value.0.facts().clone()), Some(value.1), value.2)
        });
        geoengine::Plan::build(geoengine::Request {
            current: tree.facts().clone(),
            previous,
            constraints: limits.is_none_or(|limits| limits != constraints),
            demand: sources(demand),
            prior: sources(prior),
        })
    }

    pub(in crate::layout) fn audit(prediction: Prediction, delta: &delta::Layout) -> Self {
        Self(geoengine::Impact::audit(prediction, delta.changed()))
    }
}

fn sources(nodes: Option<&[Node]>) -> Option<Arc<[geoengine::Source]>> {
    nodes.map(|nodes| {
        nodes
            .iter()
            .map(|node| node.source())
            .collect::<Vec<_>>()
            .into()
    })
}

pub(in crate::layout) struct Prior {
    snapshot: Arc<super::snapshot::Snapshot>,
    measurement: super::Generation,
    affected: Arc<[Id]>,
    planned: usize,
}

impl Prior {
    pub fn new(
        snapshot: Arc<super::snapshot::Snapshot>,
        measurement: super::Generation,
        affected: Arc<[Id]>,
    ) -> Self {
        Self {
            snapshot,
            measurement,
            affected,
            planned: 0,
        }
    }

    pub fn find(
        &mut self,
        id: Id,
        remaining: &mut usize,
        bindings: &mut super::snapshot::bindings::Bindings,
    ) -> Result<Option<Arc<crate::layout::cache::Measurement>>, crate::Fault> {
        let geometry = self
            .affected
            .binary_search(&id)
            .is_err()
            .then(|| self.measurement.find(&self.snapshot, id))
            .flatten();
        if let Some(geometry) = &geometry {
            self.snapshot.pin(geometry, bindings)?;
            self.planned += 1;
            *remaining = remaining
                .checked_sub(geometry.work.saturating_sub(1))
                .ok_or_else(|| crate::Fault("layout work budget exhausted".into()))?;
        }
        Ok(geometry)
    }

    pub fn planned(&self) -> usize {
        self.planned
    }
}
