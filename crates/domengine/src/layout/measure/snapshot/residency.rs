use super::Snapshot;
use crate::layout::{Branch, Layout, Tree, cache::Measurement, input::Element};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Weak};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct References {
    pub strong: usize,
    pub weak: usize,
}

impl References {
    fn inspect<T>(value: &Arc<T>) -> Self {
        Self {
            strong: Arc::strong_count(value),
            weak: Arc::weak_count(value),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Memory {
    pub geometry: usize,
    pub projection: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Liveness {
    pub capture: usize,
    pub tree: usize,
    pub snapshot: usize,
    pub geometry: usize,
    pub branch: usize,
}

#[derive(Clone)]
pub struct Lifetime {
    capture: Weak<Element>,
    tree: Weak<Tree>,
    snapshot: Weak<Snapshot>,
    geometry: Weak<Measurement>,
    branch: Weak<Branch>,
}

impl Lifetime {
    pub fn inspect(&self) -> Liveness {
        Liveness {
            capture: self.capture.strong_count(),
            tree: self.tree.strong_count(),
            snapshot: self.snapshot.strong_count(),
            geometry: self.geometry.strong_count(),
            branch: self.branch.strong_count(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Residency {
    pub capture: References,
    pub tree: References,
    pub snapshot: References,
    pub geometry: References,
    pub branch: References,
    pub records: usize,
    pub live: usize,
    pub states: usize,
    pub geometries: usize,
    pub branches: usize,
    pub fragments: usize,
    pub bytes: Memory,
}

impl Residency {
    pub(in crate::layout) fn inspect(layout: &Layout) -> Self {
        let snapshot = &layout.snapshot;
        let tree = snapshot.tree();
        let geometry = layout.measurement().root();
        let branch = layout.root();
        let (branches, fragments, branch_bytes) = projection(branch);
        Self {
            capture: References::inspect(tree.element()),
            tree: References::inspect(tree),
            snapshot: References::inspect(snapshot),
            geometry: References::inspect(geometry),
            branch: References::inspect(branch),
            records: tree.nodes().len(),
            live: (0..tree.nodes().len())
                .filter(|slot| tree.entry(*slot).record().alive())
                .count(),
            states: snapshot.states().count(),
            geometries: snapshot.metrics().count(),
            branches,
            fragments,
            bytes: Memory {
                geometry: geometry.cost().total(),
                projection: branch_bytes,
            },
        }
    }
}

impl Lifetime {
    pub(in crate::layout) fn new(layout: &Layout) -> Self {
        Self {
            capture: Arc::downgrade(layout.tree().element()),
            tree: Arc::downgrade(layout.tree()),
            snapshot: Arc::downgrade(layout.snapshot()),
            geometry: Arc::downgrade(layout.measurement().root()),
            branch: Arc::downgrade(layout.root()),
        }
    }
}

fn projection(branch: &Branch) -> (usize, usize, usize) {
    let mut branches = 1usize;
    let mut fragments = branch.fragments.len();
    let mut bytes = std::mem::size_of::<Branch>()
        .saturating_add(allocated(&branch.bindings))
        .saturating_add(allocated(&branch.fragments))
        .saturating_add(allocated(&branch.links))
        .saturating_add(branch.targets.as_ref().map_or(0, allocated));
    for link in &branch.links {
        let nested = projection(&link.branch);
        branches = branches.saturating_add(nested.0);
        fragments = fragments.saturating_add(nested.1);
        bytes = bytes.saturating_add(nested.2);
    }
    (branches, fragments, bytes)
}

fn allocated<T>(values: &Vec<T>) -> usize {
    values.capacity().saturating_mul(std::mem::size_of::<T>())
}
