mod collection;
mod picture;
mod tag;
mod tree;
use crate::Fault;
use crate::change::{Attribute, Fact, Journal};
pub use collection::Collection;
use picture::Binding;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Node {
    owner: u64,
    slot: usize,
    id: usize,
}

impl Node {
    pub fn id(self) -> usize {
        self.id
    }

    pub fn slot(self) -> usize {
        self.slot
    }

    pub(crate) fn source(self) -> geoengine::Source {
        geoengine::Source::new(self.owner, self.slot as u64, self.id as u64)
    }
}

#[derive(Debug, Clone)]
pub enum Kind {
    Element(String),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub kind: Kind,
    pub attributes: BTreeMap<String, String>,
    pub parent: Option<Node>,
    pub children: Vec<Node>,
}

#[derive(Clone, Copy)]
pub(crate) struct Parts {
    pub text: Node,
    pub caret: Node,
}

pub struct Document {
    owner: u64,
    pictures: BTreeMap<Node, Binding>,
    nodes: Vec<Entry>,
    ids: Vec<usize>,
    live: Vec<bool>,
    vacant: Vec<usize>,
    collection: Collection,
    next: usize,
    pub(crate) journal: Journal,
    pub(crate) editors: BTreeMap<Node, crate::Buffer>,
    pub(crate) parts: BTreeMap<Node, Parts>,
    pub(crate) focused: Option<Node>,
}

impl Default for Document {
    fn default() -> Self {
        let owner = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("document identity exhausted");
        Self {
            owner,
            pictures: BTreeMap::new(),
            journal: Journal::default(),
            editors: BTreeMap::new(),
            parts: BTreeMap::new(),
            focused: None,
            nodes: vec![Entry {
                kind: Kind::Element("document".into()),
                attributes: BTreeMap::new(),
                parent: None,
                children: Vec::new(),
            }],
            ids: vec![0],
            live: vec![true],
            vacant: Vec::new(),
            collection: Collection::default(),
            next: 0,
        }
    }
}

impl Document {
    pub fn owns(&self, node: Node) -> bool {
        node.owner == self.owner
    }

    pub fn root(&self) -> Node {
        Node {
            owner: self.owner,
            slot: 0,
            id: 0,
        }
    }

    pub fn entry(&self, node: Node) -> Result<&Entry, Fault> {
        if node.owner != self.owner {
            return Err(Fault("node belongs to another document".into()));
        }
        if !self.live.get(node.slot).copied().unwrap_or(false) || self.ids[node.slot] != node.id {
            return Err(Fault("unknown node".into()));
        }
        Ok(&self.nodes[node.slot])
    }

    pub fn element(&mut self, name: &str) -> Result<Node, Fault> {
        if !tag::supported(name) {
            return Err(Fault(format!("unsupported element: {name}")));
        }
        let node = self.allocate(Kind::Element(name.into()))?;
        if name == "input" {
            self.editors.insert(node, crate::Buffer::default());
        }
        Ok(node)
    }

    pub fn text(&mut self, text: &str) -> Result<Node, Fault> {
        if text.len() > 65_536 {
            return Err(Fault("text budget exhausted".into()));
        }
        self.allocate(Kind::Text(text.into()))
    }

    fn allocate(&mut self, kind: Kind) -> Result<Node, Fault> {
        if self.nodes.len() >= 4096 && self.vacant.is_empty() {
            return Err(Fault("node budget exhausted".into()));
        }
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| Fault("node identity exhausted".into()))?;
        let entry = Entry {
            kind,
            attributes: BTreeMap::new(),
            parent: None,
            children: Vec::new(),
        };
        let slot = if let Some(slot) = self.vacant.pop() {
            self.nodes[slot] = entry;
            self.ids[slot] = self.next;
            self.live[slot] = true;
            slot
        } else {
            let slot = self.nodes.len();
            self.nodes.push(entry);
            self.ids.push(self.next);
            self.live.push(true);
            slot
        };
        let node = Node {
            owner: self.owner,
            slot,
            id: self.next,
        };
        Ok(node)
    }

    pub fn attribute(&mut self, node: Node, name: &str, value: &str) -> Result<(), Fault> {
        if !matches!(self.entry(node)?.kind, Kind::Element(_)) {
            return Err(Fault("text has no attributes".into()));
        }
        let attribute = Attribute::parse(name)?;
        if value.len() > 4096 {
            return Err(Fault("attribute budget exhausted".into()));
        }
        if self
            .entry(node)?
            .attributes
            .get(name)
            .is_some_and(|old| old == value)
        {
            return Ok(());
        }
        let connected = self.connected(node)?;
        self.nodes[node.slot]
            .attributes
            .insert(name.into(), value.into());
        if connected {
            self.record(Fact::Attribute {
                node,
                name: attribute,
            });
        }
        Ok(())
    }

    pub fn append(&mut self, parent: Node, child: Node) -> Result<(), Fault> {
        self.entry(child)?;
        if matches!(&self.entry(parent)?.kind, Kind::Element(tag) if tag == "img") {
            return Err(Fault("img cannot contain children".into()));
        }
        if child == self.root() {
            return Err(Fault("document root cannot be reparented".into()));
        }
        if !matches!(self.entry(parent)?.kind, Kind::Element(_)) {
            return Err(Fault("text cannot contain children".into()));
        }
        if self.editors.contains_key(&parent) && !self.parts.contains_key(&parent) {
            return Err(Fault("input cannot contain DOM children".into()));
        }
        let (depth, connected) = self.attachment(parent, child)?;
        if depth + self.height(child)? > 64 {
            return Err(Fault("tree depth budget exhausted".into()));
        }
        self.detach(child)?;
        self.nodes[parent.slot].children.push(child);
        self.nodes[child.slot].parent = Some(parent);
        if self
            .focused
            .is_some_and(|node| !self.connected(node).unwrap_or(false))
        {
            self.focus(None)?;
        }
        if connected {
            self.record(Fact::Attached {
                node: child,
                from: None,
                to: Some(parent),
            });
        }
        Ok(())
    }

    pub fn remove(&mut self, parent: Node, child: Node) -> Result<(), Fault> {
        self.entry(parent)?;
        if self.entry(child)?.parent != Some(parent) {
            return Err(Fault("node is not a child".into()));
        }
        self.detach(child)?;
        if self
            .focused
            .is_some_and(|node| !self.connected(node).unwrap_or(false))
        {
            self.focus(None)?;
        }
        Ok(())
    }

    fn detach(&mut self, node: Node) -> Result<(), Fault> {
        if let Some(parent) = self.entry(node)?.parent {
            let connected = self.connected(node)?;
            self.nodes[parent.slot]
                .children
                .retain(|child| *child != node);
            self.nodes[node.slot].parent = None;
            if connected {
                self.record(Fact::Attached {
                    node,
                    from: Some(parent),
                    to: None,
                });
            }
        }
        Ok(())
    }

    pub fn replace(&mut self, node: Node, text: &str) -> Result<(), Fault> {
        if matches!(&self.entry(node)?.kind, Kind::Element(tag) if tag == "img") {
            return Err(Fault("img cannot contain text".into()));
        }
        self.entry(node)?;
        if self.editors.contains_key(&node) {
            return Err(Fault("input text uses its value, not DOM children".into()));
        }
        if text.len() > 65_536 {
            return Err(Fault("text budget exhausted".into()));
        }
        if let Kind::Text(old) = &self.nodes[node.slot].kind {
            if old == text {
                return Ok(());
            }
            let connected = self.connected(node)?;
            self.nodes[node.slot].kind = Kind::Text(text.into());
            if connected {
                self.record(Fact::Text { node });
            }
        } else {
            let child = self.text(text)?;
            for old in self.nodes[node.slot].children.clone() {
                self.remove(node, old)?;
            }
            self.append(node, child)?;
        }
        Ok(())
    }
}
