use crate::layout::{Branch, Id};
use std::{
    collections::HashMap,
    sync::{Arc, OnceLock},
};

use super::Survey;

#[derive(Debug, Clone)]
pub struct Projection {
    data: OnceLock<Data>,
    current: Artifact,
    history: Option<Artifact>,
}

#[derive(Debug, Clone)]
struct Data {
    changed: Arc<[Id]>,
    added: Arc<[Id]>,
    updated: Arc<[Id]>,
    relocated: Arc<[Id]>,
    reordered: Arc<[Id]>,
    retired: Arc<[Id]>,
    survey: Survey,
}

impl Projection {
    pub fn changed(&self) -> &[Id] {
        &self.data().changed
    }
    pub fn added(&self) -> &[Id] {
        &self.data().added
    }
    pub fn updated(&self) -> &[Id] {
        &self.data().updated
    }
    pub fn relocated(&self) -> &[Id] {
        &self.data().relocated
    }
    pub fn reordered(&self) -> &[Id] {
        &self.data().reordered
    }
    pub fn retired(&self) -> &[Id] {
        &self.data().retired
    }
    pub fn survey(&self) -> Survey {
        self.data().survey
    }

    pub(in crate::layout) fn new(root: &Arc<Branch>, previous: Option<&Self>) -> Self {
        let history = previous.map(|delta| delta.current.clone());
        Self {
            data: OnceLock::new(),
            current: Artifact::new(root, history.as_ref()),
            history,
        }
    }

    fn data(&self) -> &Data {
        self.data.get_or_init(|| match &self.history {
            Some(old) => compare(&self.current.facts, &old.facts),
            None => initial(&self.current.facts),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Position {
    parent: Option<Id>,
    at: Option<usize>,
    fragment: Option<usize>,
    offset: [usize; 2],
}

#[derive(Clone)]
struct Fact {
    id: Id,
    token: Arc<()>,
    position: Position,
}

#[derive(Clone)]
struct Artifact {
    token: Arc<()>,
    facts: Arc<[Fact]>,
}

impl Artifact {
    fn new(root: &Arc<Branch>, previous: Option<&Self>) -> Self {
        if let Some(previous) = previous
            && Arc::ptr_eq(&root.subtree, &previous.token)
        {
            return previous.clone();
        }
        let mut facts = Vec::new();
        append(
            root,
            Position {
                parent: None,
                at: None,
                fragment: None,
                offset: [0; 2],
            },
            &mut facts,
        );
        Self {
            token: root.subtree.clone(),
            facts: facts.into(),
        }
    }
}

impl std::fmt::Debug for Artifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Artifact")
            .field("branches", &self.facts.len())
            .finish()
    }
}

fn append(branch: &Arc<Branch>, position: Position, facts: &mut Vec<Fact>) {
    facts.push(Fact {
        id: branch.id(),
        token: branch.token.clone(),
        position,
    });
    let parent = Some(branch.id());
    for link in &branch.links {
        append(
            &link.branch,
            Position {
                parent,
                at: Some(link.at),
                fragment: link.parent,
                offset: link.offset,
            },
            facts,
        );
    }
}

fn initial(facts: &[Fact]) -> Data {
    let added = facts.iter().map(|fact| fact.id).collect::<Arc<_>>();
    Data {
        changed: added.clone(),
        added,
        updated: Arc::from([]),
        relocated: Arc::from([]),
        reordered: Arc::from([]),
        retired: Arc::from([]),
        survey: Survey::default(),
    }
}

fn compare(current: &[Fact], previous: &[Fact]) -> Data {
    if std::ptr::eq(current.as_ptr(), previous.as_ptr()) && current.len() == previous.len() {
        return unchanged(Survey::default());
    }
    let mut matched = 0;
    let same = current.len() == previous.len()
        && current.iter().zip(previous).all(|(current, previous)| {
            matched += 1;
            current.id == previous.id
                && Arc::ptr_eq(&current.token, &previous.token)
                && current.position == previous.position
        });
    if same {
        return unchanged(Survey {
            compared: matched,
            ..Survey::default()
        });
    }
    let old = previous
        .iter()
        .enumerate()
        .map(|(slot, fact)| (fact.id, slot))
        .collect::<HashMap<_, _>>();
    let live = current
        .iter()
        .enumerate()
        .map(|(slot, fact)| (fact.id, slot))
        .collect::<HashMap<_, _>>();
    let order = ranks(current, &old, previous);
    let prior = ranks(previous, &live, current);
    let mut changed = Vec::new();
    let mut added = Vec::new();
    let mut updated = Vec::new();
    let mut relocated = Vec::new();
    let mut reordered = Vec::new();
    for (slot, fact) in current.iter().enumerate() {
        let Some(oldslot) = old.get(&fact.id).copied() else {
            added.push(fact.id);
            changed.push(fact.id);
            continue;
        };
        let old = &previous[oldslot];
        let revised = !Arc::ptr_eq(&fact.token, &old.token);
        let shifted = fact.position != old.position;
        let ordered = order[slot] != prior[oldslot];
        push(&mut updated, fact.id, revised);
        push(&mut relocated, fact.id, shifted);
        push(&mut reordered, fact.id, ordered);
        push(&mut changed, fact.id, revised || shifted || ordered);
    }
    let retired = previous
        .iter()
        .filter_map(|fact| (!live.contains_key(&fact.id)).then_some(fact.id))
        .collect::<Vec<_>>();
    Data {
        changed: changed.into(),
        added: added.into(),
        updated: updated.into(),
        relocated: relocated.into(),
        reordered: reordered.into(),
        retired: retired.into(),
        survey: Survey {
            compared: matched + current.len(),
            scanned: previous.len(),
            indexed: current.len() + previous.len(),
        },
    }
}

fn unchanged(survey: Survey) -> Data {
    Data {
        changed: Arc::from([]),
        added: Arc::from([]),
        updated: Arc::from([]),
        relocated: Arc::from([]),
        reordered: Arc::from([]),
        retired: Arc::from([]),
        survey,
    }
}

fn ranks(facts: &[Fact], other: &HashMap<Id, usize>, peers: &[Fact]) -> Vec<usize> {
    let mut positions = HashMap::<Option<Id>, usize>::new();
    let mut result = vec![usize::MAX; facts.len()];
    for (slot, fact) in facts.iter().enumerate() {
        if other
            .get(&fact.id)
            .is_none_or(|slot| peers[*slot].position.parent != fact.position.parent)
        {
            continue;
        }
        let rank = positions.entry(fact.position.parent).or_default();
        result[slot] = *rank;
        *rank += 1;
    }
    result
}

fn push(target: &mut Vec<Id>, id: Id, changed: bool) {
    if changed {
        target.push(id);
    }
}
