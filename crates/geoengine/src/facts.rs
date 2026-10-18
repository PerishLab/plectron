use crate::{Error, Id, Source};
use std::{
    collections::{HashMap, VecDeque},
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

static GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Shape {
    pub nodes: usize,
    pub edges: usize,
    pub flows: usize,
    pub roots: usize,
    pub boundaries: usize,
    pub waves: usize,
    pub width: usize,
    pub fanout: usize,
}

impl Shape {
    fn build(entities: &[Entity], edges: usize, flows: &[usize], postorder: &[usize]) -> Self {
        let mut levels = vec![0usize; entities.len()];
        let mut widths = Vec::new();
        let mut fanout = 0;
        for &slot in postorder {
            let entity = entities[slot];
            let children = &flows[entity.flow()];
            fanout = fanout.max(children.len());
            let level = children
                .iter()
                .map(|child| levels[*child] + 1)
                .max()
                .unwrap_or(0);
            levels[slot] = level;
            if widths.len() <= level {
                widths.resize(level + 1, 0);
            }
            widths[level] += 1;
        }
        Self {
            nodes: entities.len(),
            edges,
            flows: flows.len(),
            roots: entities
                .iter()
                .filter(|entity| entity.parent.is_none())
                .count(),
            boundaries: entities.iter().filter(|entity| entity.boundary).count(),
            waves: widths.len(),
            width: widths.into_iter().max().unwrap_or(0),
            fanout,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Source,
    Anonymous,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entity {
    pub id: Id,
    pub version: u64,
    pub parent: Option<Id>,
    pub source: Source,
    pub role: Role,
    pub boundary: bool,
    start: usize,
    length: usize,
}

impl Entity {
    pub const fn new(
        identity: (Id, u64),
        origin: (Option<Id>, Source),
        nature: (Role, bool),
        flow: Range<usize>,
    ) -> Self {
        let (id, version) = identity;
        let (parent, source) = origin;
        let (role, boundary) = nature;
        Self {
            id,
            version,
            parent,
            source,
            role,
            boundary,
            start: flow.start,
            length: flow.end - flow.start,
        }
    }

    pub const fn flow(self) -> Range<usize> {
        self.start..self.start + self.length
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Relation {
    Intrinsic,
    Constraint,
    Flow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Edge {
    pub from: Id,
    pub to: Id,
    pub relation: Relation,
}

#[derive(Clone, Debug)]
pub struct Facts {
    revision: u64,
    generation: u64,
    root: Id,
    entities: Arc<[Entity]>,
    edges: Arc<[Edge]>,
    flows: Arc<[usize]>,
    index: Arc<HashMap<Id, usize>>,
    postorder: Arc<[usize]>,
    shape: Shape,
}

pub struct Graph {
    pub entities: Arc<[Entity]>,
    pub edges: Arc<[Edge]>,
    pub flows: Arc<[usize]>,
}

impl Facts {
    pub fn new(revision: u64, root: Id, graph: Graph) -> Result<Arc<Self>, Error> {
        let Graph {
            entities,
            edges,
            flows,
        } = graph;
        let mut index = HashMap::with_capacity(entities.len());
        for (slot, entity) in entities.iter().enumerate() {
            if index.insert(entity.id, slot).is_some() {
                return Err(Error("duplicate geometry identity".into()));
            }
            if entity.flow().end > flows.len() {
                return Err(Error("geometry flow is outside its facts".into()));
            }
        }
        if !index.contains_key(&root) {
            return Err(Error("geometry root is absent".into()));
        }
        if flows.iter().any(|slot| *slot >= entities.len()) {
            return Err(Error("geometry flow references an absent entity".into()));
        }
        if entities
            .iter()
            .filter_map(|entity| entity.parent)
            .any(|id| !index.contains_key(&id))
            || edges
                .iter()
                .any(|edge| !index.contains_key(&edge.from) || !index.contains_key(&edge.to))
        {
            return Err(Error(
                "geometry relation references an absent entity".into(),
            ));
        }
        let postorder = order(&entities, &flows, &index)?;
        let shape = Shape::build(&entities, edges.len(), &flows, &postorder);
        Ok(Arc::new(Self {
            revision,
            generation: GENERATION
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                    value.checked_add(1)
                })
                .expect("geometry generation exhausted"),
            root,
            entities,
            edges,
            flows,
            index: Arc::new(index),
            postorder: postorder.into(),
            shape,
        }))
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn root(&self) -> Id {
        self.root
    }

    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn entity(&self, id: Id) -> Option<&Entity> {
        self.slot(id).map(|slot| &self.entities[slot])
    }

    pub fn slot(&self, id: Id) -> Option<usize> {
        self.index.get(&id).copied()
    }

    pub fn children(&self, slot: usize) -> impl Iterator<Item = usize> + '_ {
        self.childslots(slot).iter().copied()
    }

    pub fn roots(&self) -> impl Iterator<Item = &Entity> {
        self.entities
            .iter()
            .filter(|entity| entity.parent.is_none())
    }

    pub const fn shape(&self) -> Shape {
        self.shape
    }

    pub(crate) fn postorder(&self) -> &[usize] {
        &self.postorder
    }

    pub(crate) fn childslots(&self, slot: usize) -> &[usize] {
        let entity = self.entities[slot];
        &self.flows[entity.flow()]
    }
}

fn order(
    entities: &[Entity],
    flows: &[usize],
    index: &HashMap<Id, usize>,
) -> Result<Vec<usize>, Error> {
    let mut linked = vec![false; entities.len()];
    let mut pending = vec![0usize; entities.len()];
    let mut parents = vec![None; entities.len()];
    for (slot, entity) in entities.iter().enumerate() {
        pending[slot] = entity.length;
        parents[slot] = entity.parent.and_then(|id| index.get(&id).copied());
        for child in &flows[entity.flow()] {
            if entities[*child].parent != Some(entity.id)
                || std::mem::replace(&mut linked[*child], true)
            {
                return Err(Error(
                    "geometry flow disagrees with its parent relation".into(),
                ));
            }
        }
    }
    if entities
        .iter()
        .enumerate()
        .any(|(slot, entity)| entity.parent.is_some() != linked[slot])
    {
        return Err(Error("geometry parent is absent from its flow".into()));
    }
    let mut ready = pending
        .iter()
        .enumerate()
        .filter_map(|(slot, count)| (*count == 0).then_some(slot))
        .collect::<VecDeque<_>>();
    let mut postorder = Vec::with_capacity(entities.len());
    while let Some(slot) = ready.pop_front() {
        postorder.push(slot);
        if let Some(parent) = parents[slot] {
            pending[parent] -= 1;
            if pending[parent] == 0 {
                ready.push_back(parent);
            }
        }
    }
    if postorder.len() != entities.len() {
        return Err(Error("geometry flow contains a cycle".into()));
    }
    Ok(postorder)
}
