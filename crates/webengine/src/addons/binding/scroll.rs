use super::super::{Native, Web};
use crate::{Fault, display};
use domengine::Node;
use jsengine::{Engine, Value};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Default)]
pub(crate) struct Scrolls {
    sequence: u64,
    entries: BTreeMap<u64, Entry>,
    owners: HashMap<Node, u64>,
    replays: BTreeSet<Node>,
}

struct Entry {
    node: Node,
    action: Option<display::Action>,
    version: u64,
}

impl Scrolls {
    pub fn roots(&self, visit: &mut dyn FnMut(Node)) {
        for entry in self.entries.values() {
            visit(entry.node);
            if let Some(display::Action::Restore(bookmark)) = entry.action {
                visit(bookmark.target);
            }
        }
    }

    pub fn observe(&mut self, node: Node) {
        self.replays.insert(node);
    }

    pub fn replay(&mut self) -> Vec<Node> {
        std::mem::take(&mut self.replays).into_iter().collect()
    }

    pub fn attach(&mut self, node: Node) -> Result<u64, Fault> {
        if self.owners.contains_key(&node) {
            return Err(Fault("scroll container already has a controller".into()));
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Fault("scroll controller identifiers exhausted".into()))?;
        self.owners.insert(node, self.sequence);
        self.entries.insert(
            self.sequence,
            Entry {
                node,
                action: None,
                version: 0,
            },
        );
        Ok(self.sequence)
    }

    pub fn follow(&mut self, lease: u64) -> Result<(), Fault> {
        let entry = self
            .entries
            .get_mut(&lease)
            .ok_or_else(|| Fault("scroll controller is disposed".into()))?;
        entry.version = entry
            .version
            .checked_add(1)
            .ok_or_else(|| Fault("scroll controller versions exhausted".into()))?;
        entry.action = Some(display::Action::Follow(display::Edge::End));
        Ok(())
    }

    pub fn restore(&mut self, lease: u64, target: Node, inset: i64) -> Result<(), Fault> {
        let entry = self
            .entries
            .get_mut(&lease)
            .ok_or_else(|| Fault("scroll controller is disposed".into()))?;
        entry.version = entry
            .version
            .checked_add(1)
            .ok_or_else(|| Fault("scroll controller versions exhausted".into()))?;
        entry.action = Some(display::Action::Restore(display::Bookmark {
            target,
            inset,
        }));
        Ok(())
    }

    pub fn release(&mut self, lease: u64) {
        if let Some(entry) = self.entries.remove(&lease) {
            self.owners.remove(&entry.node);
        }
    }

    pub(crate) fn policies(&self) -> Vec<display::Policy> {
        self.entries
            .iter()
            .filter_map(|(lease, entry)| {
                entry.action.map(|action| display::Policy {
                    lease: *lease,
                    version: entry.version,
                    node: entry.node,
                    action,
                })
            })
            .collect()
    }
}

impl Web {
    pub(super) fn scrolling(&mut self, engine: &mut Engine, node: Node) -> Result<Value, Fault> {
        let lease = self.scrolls.attach(node)?;
        let follow = self.bind(engine, Native::Follow(lease, node));
        let restore = self.bind(engine, Native::Restore(lease, node));
        let dispose = self.bind(engine, Native::Unscroll(lease, node));
        engine
            .object(
                [
                    ("followEnd".into(), follow),
                    ("restore".into(), restore),
                    ("dispose".into(), dispose),
                ]
                .into(),
            )
            .map_err(crate::error)
    }
}
