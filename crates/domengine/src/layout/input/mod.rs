mod capture;
mod damage;
pub use damage::{Cause, Mode, Plan};
mod element;
mod flow;
mod key;
pub(super) use flow::{Inline, Section};
pub use geoengine::{Edge, Entity, Id, Relation, Role};
mod report;
mod shaping;
mod sizing;
use super::delta;
use crate::{
    Node,
    style::{Inherited, Style},
};
pub use capture::Capture;
pub(super) use key::{Entry, Key, Origin, Record};
pub use report::{Timing, Update};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

static VERSION: AtomicU64 = AtomicU64::new(1);

#[derive(Clone)]
pub struct Tree {
    facts: Arc<geoengine::Facts>,
    records: Arc<[Arc<Record>]>,
    element: Arc<Element>,
    delta: delta::Tree,
    plan: Plan,
}

impl std::fmt::Debug for Tree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tree")
            .field("facts", &self.facts)
            .field("plan", &self.plan)
            .finish()
    }
}

impl Tree {
    pub fn revision(&self) -> u64 {
        self.facts.revision()
    }

    pub fn generation(&self) -> u64 {
        self.facts.generation()
    }

    pub fn nodes(&self) -> &[Entity] {
        self.facts.entities()
    }

    pub fn edges(&self) -> &[Edge] {
        self.facts.edges()
    }

    pub fn root(&self) -> Id {
        self.facts.root()
    }

    pub fn entity(&self, id: Id) -> Option<&Entity> {
        self.facts.entity(id)
    }

    pub fn flow(&self, id: Id) -> Option<impl Iterator<Item = Id> + '_> {
        let slot = self.facts.slot(id)?;
        Some(self.children(slot).map(|child| self.nodes()[child].id))
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn delta(&self) -> &delta::Tree {
        self.delta.resolve((&self.facts, &self.records));
        &self.delta
    }

    pub fn roots(&self) -> impl Iterator<Item = &Entity> {
        self.facts.roots()
    }

    pub fn source(&self, source: Node) -> impl Iterator<Item = &Entity> {
        let source = source.source();
        self.nodes()
            .iter()
            .filter(move |node| node.source == source)
    }
    pub(super) fn element(&self) -> &Arc<Element> {
        &self.element
    }

    pub(in crate::layout) fn entry(&self, slot: usize) -> Entry<'_> {
        Entry { tree: self, slot }
    }

    pub(super) fn origin(&self) -> Entry<'_> {
        self.entry(self.slot(self.root()).expect("layout root"))
    }

    pub(in crate::layout) fn slot(&self, id: Id) -> Option<usize> {
        self.facts.slot(id)
    }

    pub(in crate::layout) fn children(&self, slot: usize) -> impl Iterator<Item = usize> + '_ {
        self.facts.children(slot)
    }

    fn build(
        root: &Arc<Element>,
        revision: u64,
        draft: damage::Draft,
        previous: Option<&Self>,
    ) -> (Arc<Self>, Timing) {
        let start = std::time::Instant::now();
        let flow::Topology {
            nodes,
            edges,
            flows,
            records,
        } = flow::topology(root);
        let topology = elapsed(start);
        let start = std::time::Instant::now();
        let plan = Plan::build(&nodes, draft);
        let planning = elapsed(start);
        let delta = delta::Tree::new(
            previous.map(|tree| delta::tree::Stored::new(tree.facts.clone(), tree.records.clone())),
        );
        let start = std::time::Instant::now();
        let facts = geoengine::Facts::new(
            revision,
            root.id,
            geoengine::Graph {
                entities: nodes,
                edges,
                flows,
            },
        )
        .expect("captured geometry facts");
        let construction = elapsed(start);
        (
            Arc::new(Self {
                facts,
                records,
                element: root.clone(),
                delta,
                plan,
            }),
            Timing {
                topology,
                planning,
                facts: construction,
            },
        )
    }

    pub(in crate::layout) fn facts(&self) -> &Arc<geoengine::Facts> {
        &self.facts
    }
}

fn elapsed(start: std::time::Instant) -> u64 {
    start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

#[derive(Clone)]
pub struct Input {
    pub(super) revision: u64,
    pub(super) tree: Arc<Tree>,
}

#[derive(Clone)]
pub(super) struct Element {
    pub id: Id,
    pub version: u64,
    pub key: Arc<Key>,
    pub node: Node,
    pub target: Node,
    pub style: Arc<Style>,
    pub text: Option<Arc<str>>,
    pub picture: Option<graphics::shared::Raster>,
    pub editor: Option<Editor>,
    pub children: Vec<Arc<Element>>,
    pub flow: Vec<Arc<Element>>,
    pub inline: Option<Arc<Inline>>,
    inherited: Inherited,
    count: usize,
}

impl Element {
    fn version(key: &Arc<Key>, previous: Option<&Arc<Self>>) -> u64 {
        previous
            .filter(|old| old.key.local() == key.local())
            .map_or_else(
                || {
                    VERSION
                        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                            value.checked_add(1)
                        })
                        .expect("layout version exhausted")
                },
                |old| old.version,
            )
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Editor {
    pub offsets: [usize; 2],
    pub active: bool,
    pub caret: Option<Caret>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Caret {
    pub node: Node,
    pub color: u32,
    pub opacity: u8,
    pub width: usize,
    pub height: Option<usize>,
    pub translation: [i64; 2],
}
