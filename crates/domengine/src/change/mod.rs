mod attribute;
mod retention;
use crate::{Document, Node};
pub use attribute::Attribute;
pub use retention::Retention;
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    pub(crate) root: Node,
    pub(crate) revision: u64,
}
impl Version {
    pub fn revision(self) -> u64 {
        self.revision
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fact {
    Image {
        node: Node,
    },
    Attribute {
        node: Node,
        name: Attribute,
    },
    Text {
        node: Node,
    },
    Attached {
        node: Node,
        from: Option<Node>,
        to: Option<Node>,
    },
    Focus {
        from: Option<Node>,
        to: Option<Node>,
    },
    Value {
        node: Node,
    },
    Selection {
        node: Node,
    },
    Composition {
        node: Node,
    },
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub revision: u64,
    pub fact: Fact,
}
pub(crate) struct Journal {
    revision: u64,
    entries: VecDeque<Change>,
    limit: usize,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            revision: 0,
            entries: VecDeque::new(),
            limit: retention::LIMIT,
        }
    }
}
impl Document {
    pub(crate) fn invalidate(&mut self) {
        self.journal.revision = self
            .journal
            .revision
            .checked_add(1)
            .expect("document revision exhausted");
        self.journal.entries.clear();
    }

    pub fn version(&self) -> Version {
        Version {
            root: self.root(),
            revision: self.revision(),
        }
    }
    pub fn changes(&self, from: Version) -> Option<Vec<Change>> {
        if from.root != self.root() || from.revision > self.revision() {
            return None;
        }
        let floor = self
            .journal
            .entries
            .front()
            .map_or(self.revision(), |change| change.revision - 1);
        if from.revision < floor {
            return None;
        }
        Some(
            self.journal
                .entries
                .range((from.revision - floor) as usize..)
                .cloned()
                .collect(),
        )
    }
    pub(crate) fn record(&mut self, fact: Fact) {
        let revision = self
            .journal
            .revision
            .checked_add(1)
            .expect("document revision exhausted");
        self.journal.revision = revision;
        if self.journal.limit == 0 {
            return;
        }
        if self.journal.entries.len() == self.journal.limit {
            self.journal.entries.pop_front();
        }
        self.journal.entries.push_back(Change { revision, fact });
    }
    pub fn revision(&self) -> u64 {
        self.journal.revision
    }
}
