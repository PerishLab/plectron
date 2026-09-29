mod audit;
mod change;
use crate::{Facts, Id, Relation, Source};
pub use audit::Impact;
use change::Change;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Survey {
    pub compared: usize,
    pub topology: usize,
    pub indexed: usize,
    pub routed: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topology {
    Same,
    Aligned,
    Remapped,
    Changed,
}

impl Topology {
    pub const fn scoped(self) -> bool {
        !matches!(self, Self::Changed)
    }

    pub const fn collected(self) -> bool {
        matches!(self, Self::Same)
    }
}

#[derive(Clone)]
pub struct Request {
    pub current: Arc<Facts>,
    pub previous: Option<Arc<Facts>>,
    pub constraints: bool,
    pub demand: Option<Arc<[Source]>>,
    pub prior: Option<Arc<[Source]>>,
}

#[derive(Debug, Clone)]
pub struct Plan {
    seeds: Arc<[Id]>,
    affected: Arc<[Id]>,
    publication: Arc<[Id]>,
    survey: Survey,
    topology: Topology,
}

impl Plan {
    pub fn build(request: Request) -> Self {
        let current = request.current.as_ref();
        let Some(previous) = request.previous.as_deref() else {
            return Self {
                seeds: current
                    .roots()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>()
                    .into(),
                affected: current
                    .entities()
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>()
                    .into(),
                publication: current
                    .entities()
                    .iter()
                    .map(|entity| entity.id)
                    .collect::<Vec<_>>()
                    .into(),
                survey: Survey {
                    compared: current.entities().len(),
                    ..Survey::default()
                },
                topology: Topology::Changed,
            };
        };
        let Change {
            mut direct,
            mut survey,
            topology,
        } = Change::between(current, previous);
        let mut queue = direct
            .iter()
            .copied()
            .map(|id| (id, Path::Direct))
            .collect::<VecDeque<_>>();
        if request.constraints {
            for root in current.roots() {
                direct.insert(root.id);
                queue.push_back((root.id, Path::Constraint));
            }
        }
        if request.demand != request.prior {
            for entity in current.entities().iter().filter(|entity| {
                entity.boundary
                    && active(request.demand.as_deref(), entity.source)
                        != active(request.prior.as_deref(), entity.source)
            }) {
                direct.insert(entity.id);
                queue.push_back((entity.id, Path::Demand));
            }
        }
        if queue.is_empty() {
            return Self {
                seeds: Arc::from([]),
                affected: Arc::from([]),
                publication: Arc::from([]),
                survey,
                topology,
            };
        }
        let live = current
            .entities()
            .iter()
            .map(|entity| (entity.id, entity))
            .collect::<BTreeMap<_, _>>();
        let mut seen = BTreeSet::new();
        let mut affected = BTreeSet::new();
        let mut outgoing = BTreeMap::<Id, Vec<_>>::new();
        for edge in current.edges() {
            outgoing.entry(edge.from).or_default().push(edge);
            survey.indexed += 1;
        }
        while let Some((id, path)) = queue.pop_front() {
            if !seen.insert((id, path)) {
                continue;
            }
            affected.insert(id);
            if path == Path::Intrinsic && live.get(&id).is_some_and(|entity| entity.boundary) {
                continue;
            }
            for edge in outgoing.get(&id).into_iter().flatten() {
                survey.routed += 1;
                let next = match (path, edge.relation) {
                    (Path::Direct, relation) => Path::from(relation),
                    (Path::Intrinsic, Relation::Intrinsic) => Path::Intrinsic,
                    (Path::Intrinsic, Relation::Flow) | (Path::Flow, Relation::Flow) => Path::Flow,
                    (Path::Constraint, Relation::Constraint) => Path::Constraint,
                    (Path::Demand, Relation::Constraint) => Path::Demand,
                    _ => continue,
                };
                queue.push_back((edge.to, next));
            }
        }
        let mut publication = affected.clone();
        for mut id in affected.iter().copied() {
            while let Some(parent) = live.get(&id).and_then(|entity| entity.parent) {
                publication.insert(parent);
                id = parent;
            }
        }
        Self {
            seeds: direct.into_iter().collect::<Vec<_>>().into(),
            affected: affected.into_iter().collect::<Vec<_>>().into(),
            publication: publication.into_iter().collect::<Vec<_>>().into(),
            survey,
            topology,
        }
    }

    pub fn seeds(&self) -> &[Id] {
        &self.seeds
    }

    pub fn affected(&self) -> &Arc<[Id]> {
        &self.affected
    }

    pub fn publication(&self) -> &Arc<[Id]> {
        &self.publication
    }

    pub const fn survey(&self) -> Survey {
        self.survey
    }

    pub const fn topology(&self) -> Topology {
        self.topology
    }

    pub fn scope(&self) -> Option<&[Id]> {
        self.topology.scoped().then_some(&self.affected)
    }

    pub fn collection(&self) -> Option<&[Id]> {
        self.topology.collected().then_some(&self.affected)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Path {
    Direct,
    Intrinsic,
    Constraint,
    Demand,
    Flow,
}

impl From<Relation> for Path {
    fn from(value: Relation) -> Self {
        match value {
            Relation::Intrinsic => Self::Intrinsic,
            Relation::Constraint => Self::Constraint,
            Relation::Flow => Self::Flow,
        }
    }
}

fn active(demand: Option<&[Source]>, source: Source) -> bool {
    demand.is_none_or(|sources| sources.contains(&source))
}
