use crate::{
    Document, Fault, Node,
    change::{Attribute, Change, Fact},
};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Full,
    Delta,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Cause {
    Style,
    Content,
    Tree,
    Resource,
    Edit,
    Focus,
    Effect,
}

#[derive(Debug, Clone)]
pub struct Plan {
    mode: Mode,
    causes: std::sync::Arc<[Cause]>,
    seeds: std::sync::Arc<[super::Id]>,
    frontier: std::sync::Arc<[super::Id]>,
}

impl Plan {
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn causes(&self) -> &[Cause] {
        &self.causes
    }
    pub fn seeds(&self) -> &[super::Id] {
        &self.seeds
    }
    pub fn frontier(&self) -> &[super::Id] {
        &self.frontier
    }
    pub(super) fn build(nodes: &[super::Entity], draft: Draft) -> Self {
        let ids = |sources: &Nodes| {
            nodes
                .iter()
                .filter(|node| sources.owns(node.source))
                .map(|node| node.id)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>()
        };
        let (seeds, frontier) = if draft.mode == Mode::Full {
            (
                nodes
                    .iter()
                    .filter(|node| node.parent.is_none())
                    .map(|node| node.id)
                    .collect(),
                nodes.iter().map(|node| node.id).collect(),
            )
        } else {
            (ids(&draft.seeds), ids(&draft.frontier))
        };
        Self {
            mode: draft.mode,
            causes: draft.causes.into_iter().collect(),
            seeds: seeds.into(),
            frontier: frontier.into(),
        }
    }
}

pub(super) struct Draft {
    mode: Mode,
    causes: BTreeSet<Cause>,
    seeds: Nodes,
    frontier: Nodes,
}

impl Draft {
    pub fn full() -> Self {
        Self {
            mode: Mode::Full,
            causes: BTreeSet::new(),
            seeds: Nodes::default(),
            frontier: Nodes::default(),
        }
    }
}

#[derive(Default)]
pub(super) struct Damage {
    pub nodes: Nodes,
    pub styles: Nodes,
    pub ancestry: usize,
    seeds: Nodes,
    causes: BTreeSet<Cause>,
}
impl Damage {
    pub fn draft(self) -> Draft {
        Draft {
            mode: Mode::Delta,
            causes: self.causes,
            seeds: self.seeds,
            frontier: self.nodes,
        }
    }
    pub fn collect(document: &Document, changes: &[Change]) -> Result<Self, Fault> {
        let mut damage = Self::default();
        for change in changes {
            if let Fact::Attribute {
                node,
                name: Attribute::Src,
            } = change.fact
            {
                if matches!(&document.entry(node)?.kind, crate::Kind::Element(tag) if tag == "img")
                {
                    damage.causes.insert(Cause::Resource);
                    damage.seeds.insert(node);
                    damage.nodes.insert(node);
                }
            } else {
                damage.record(&change.fact);
            }
        }
        let nodes = damage.nodes.take();
        let mut paths = Paths::default();
        for node in nodes.into_iter().flatten() {
            damage.touch(document, node, &mut paths)?;
        }
        Ok(damage)
    }
    pub fn animate(
        &mut self,
        document: &Document,
        nodes: impl IntoIterator<Item = Node>,
    ) -> Result<(), Fault> {
        let nodes = nodes.into_iter().collect::<Vec<_>>();
        if !nodes.is_empty() {
            self.causes.insert(Cause::Effect);
        }
        let mut paths = Paths::default();
        for node in nodes {
            self.seeds.insert(node);
            let connected = paths.connect(document, node, &self.nodes)?;
            self.ancestry += paths.pending.len();
            if connected {
                self.nodes.extend(paths.pending.drain(..));
            } else {
                paths.detached.extend(paths.pending.drain(..));
            }
        }
        Ok(())
    }
    fn record(&mut self, fact: &Fact) {
        match *fact {
            Fact::Attribute {
                node,
                name: Attribute::Id | Attribute::Class | Attribute::Style | Attribute::Dir,
            } => {
                self.causes.insert(Cause::Style);
                self.seeds.insert(node);
                self.styles.insert(node);
                self.nodes.insert(node);
            }
            Fact::Image { node } => {
                self.causes.insert(Cause::Resource);
                self.seeds.insert(node);
                self.nodes.insert(node);
            }
            Fact::Text { node } => {
                self.causes.insert(Cause::Content);
                self.seeds.insert(node);
                self.nodes.insert(node);
            }
            Fact::Value { node } | Fact::Selection { node } | Fact::Composition { node } => {
                self.causes.insert(Cause::Edit);
                self.seeds.insert(node);
                self.nodes.insert(node);
            }
            Fact::Attached { node, from, to } => {
                self.causes.insert(Cause::Tree);
                self.seeds.insert(node);
                self.seeds.extend(from.into_iter().chain(to));
                self.nodes.insert(node);
                self.nodes.extend(from.into_iter().chain(to));
            }
            Fact::Focus { from, to } => {
                self.causes.insert(Cause::Focus);
                self.seeds.extend(from.into_iter().chain(to));
                self.nodes.extend(from.into_iter().chain(to));
            }
            Fact::Attribute {
                name: Attribute::Src | Attribute::Href | Attribute::Rel,
                ..
            } => (),
        }
    }
    fn touch(&mut self, document: &Document, node: Node, paths: &mut Paths) -> Result<(), Fault> {
        let connected = paths.connect(document, node, &self.nodes)?;
        self.ancestry += paths.pending.len();
        if connected {
            self.nodes.extend(paths.pending.drain(..));
        } else {
            paths.detached.extend(paths.pending.drain(..));
        }
        Ok(())
    }
}

#[derive(Default)]
struct Paths {
    detached: Nodes,
    pending: Vec<Node>,
}
impl Paths {
    fn connect(
        &mut self,
        document: &Document,
        mut node: Node,
        connected: &Nodes,
    ) -> Result<bool, Fault> {
        loop {
            if connected.contains(&node) {
                return Ok(true);
            }
            if self.detached.contains(&node) {
                return Ok(false);
            }
            self.pending.push(node);
            let parent = document.entry(node)?.parent;
            if node == document.root() {
                return Ok(true);
            }
            let Some(parent) = parent else {
                return Ok(false);
            };
            node = parent;
        }
    }
}

#[derive(Default)]
pub(super) struct Nodes {
    slots: Vec<Option<Node>>,
    length: usize,
}
impl Nodes {
    fn insert(&mut self, node: Node) {
        if self.slots.len() <= node.slot() {
            self.slots.resize(node.slot() + 1, None);
        }
        let slot = &mut self.slots[node.slot()];
        if *slot != Some(node) {
            self.length += usize::from(slot.is_none());
            *slot = Some(node);
        }
    }
    fn extend(&mut self, nodes: impl IntoIterator<Item = Node>) {
        for node in nodes {
            self.insert(node);
        }
    }
    pub fn contains(&self, node: &Node) -> bool {
        self.slots.get(node.slot()).copied().flatten() == Some(*node)
    }
    fn owns(&self, source: geoengine::Source) -> bool {
        usize::try_from(source.slot())
            .ok()
            .and_then(|slot| self.slots.get(slot))
            .copied()
            .flatten()
            .is_some_and(|node| node.source() == source)
    }
    pub fn len(&self) -> usize {
        self.length
    }
    fn take(&mut self) -> Vec<Option<Node>> {
        self.length = 0;
        std::mem::take(&mut self.slots)
    }
}
