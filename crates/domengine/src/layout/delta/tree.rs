use crate::layout::{
    Id,
    input::{Entity, Record},
};
use std::sync::{Arc, OnceLock};

type History<'a> = (&'a geoengine::Facts, &'a [Arc<Record>]);

#[derive(Clone)]
pub struct Tree {
    data: OnceLock<Data>,
    history: Option<Stored>,
}

#[derive(Debug, Clone)]
struct Data {
    changed: Arc<[Id]>,
    added: Arc<[Id]>,
    updated: Arc<[Id]>,
    moved: Arc<[Id]>,
    reordered: Arc<[Id]>,
    retired: Arc<[Id]>,
}

impl Tree {
    pub fn changed(&self) -> &[Id] {
        &self.data().changed
    }
    pub fn added(&self) -> &[Id] {
        &self.data().added
    }
    pub fn updated(&self) -> &[Id] {
        &self.data().updated
    }
    pub fn moved(&self) -> &[Id] {
        &self.data().moved
    }
    pub fn reordered(&self) -> &[Id] {
        &self.data().reordered
    }
    pub fn retired(&self) -> &[Id] {
        &self.data().retired
    }

    pub(in crate::layout) fn new(previous: Option<Stored>) -> Self {
        Self {
            data: OnceLock::new(),
            history: previous,
        }
    }

    pub(in crate::layout) fn resolve(&self, current: History<'_>) {
        self.data.get_or_init(|| match &self.history {
            Some(old) => compare(current, old.view()),
            None => initial(current.0.entities()),
        });
    }

    fn data(&self) -> &Data {
        self.data.get().expect("tree delta is resolved")
    }
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Tree")
            .field("data", &self.data.get())
            .finish()
    }
}

#[derive(Clone)]
pub(in crate::layout) struct Stored {
    facts: Arc<geoengine::Facts>,
    records: Arc<[Arc<Record>]>,
}

impl Stored {
    pub(in crate::layout) fn new(
        facts: Arc<geoengine::Facts>,
        records: Arc<[Arc<Record>]>,
    ) -> Self {
        Self { facts, records }
    }

    fn view(&self) -> History<'_> {
        (&self.facts, &self.records)
    }
}

fn initial(nodes: &[Entity]) -> Data {
    let added = nodes.iter().map(|entity| entity.id).collect::<Arc<_>>();
    Data {
        changed: added.clone(),
        added,
        updated: Arc::from([]),
        moved: Arc::from([]),
        reordered: Arc::from([]),
        retired: Arc::from([]),
    }
}

fn compare(current: History<'_>, previous: History<'_>) -> Data {
    let (facts, records) = current;
    let (oldfacts, oldrecords) = previous;
    let nodes = facts.entities();
    let oldnodes = oldfacts.entities();
    let current = ranks(facts, |entity| {
        oldfacts
            .slot(entity.id)
            .is_some_and(|slot| oldnodes[slot].parent == entity.parent)
    });
    let prior = ranks(oldfacts, |entity| {
        facts
            .slot(entity.id)
            .is_some_and(|slot| nodes[slot].parent == entity.parent)
    });
    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut moved = Vec::new();
    let mut reordered = Vec::new();
    for (slot, (entity, record)) in nodes.iter().zip(records).enumerate() {
        let Some(oldslot) = oldfacts.slot(entity.id) else {
            added.push(entity.id);
            changed.push(entity.id);
            continue;
        };
        let old = &oldnodes[oldslot];
        let oldrecord = &oldrecords[oldslot];
        let shifted = entity.parent != old.parent;
        let origin = entity.source != old.source
            || entity.role != old.role
            || entity.boundary != old.boundary;
        let record = record.matches(oldrecord) || record.same(oldrecord);
        let revised = entity.version != old.version || origin || !record;
        let ordered = current[slot] != prior[oldslot];
        push(&mut moved, entity.id, shifted);
        push(&mut updated, entity.id, revised);
        push(&mut reordered, entity.id, ordered);
        push(&mut changed, entity.id, shifted || revised || ordered);
    }
    let retired = oldnodes
        .iter()
        .filter_map(|entity| facts.slot(entity.id).is_none().then_some(entity.id))
        .collect::<Vec<_>>();
    Data {
        changed: changed.into(),
        added: added.into(),
        updated: updated.into(),
        moved: moved.into(),
        reordered: reordered.into(),
        retired: retired.into(),
    }
}

fn ranks(facts: &geoengine::Facts, include: impl Fn(&Entity) -> bool) -> Vec<usize> {
    let nodes = facts.entities();
    let mut counts = vec![0; nodes.len() + 1];
    let mut result = vec![usize::MAX; nodes.len()];
    for (slot, entity) in nodes
        .iter()
        .enumerate()
        .filter(|(_, entity)| include(entity))
    {
        let parent = entity
            .parent
            .and_then(|id| facts.slot(id))
            .map_or(0, |slot| slot + 1);
        result[slot] = counts[parent];
        counts[parent] += 1;
    }
    result
}

fn push(target: &mut Vec<Id>, id: Id, changed: bool) {
    if changed {
        target.push(id);
    }
}
