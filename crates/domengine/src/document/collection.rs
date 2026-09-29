use super::{Document, Entry, Kind, Node};
use std::collections::{BTreeMap, HashSet};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Collection {
    pub live: usize,
    pub released: usize,
    pub capacity: usize,
    pub vacant: usize,
    pub allocations: usize,
}

impl Document {
    pub fn allocations(&self) -> usize {
        self.next
    }

    pub fn collection(&self) -> Collection {
        self.collection
    }

    pub fn collect(&mut self, roots: impl IntoIterator<Item = Node>) -> Vec<Node> {
        match observe::call("dom.collect", || {
            Ok::<_, std::convert::Infallible>(self.reclaim(roots))
        }) {
            Ok(dead) => dead,
            Err(never) => match never {},
        }
    }

    fn reclaim(&mut self, roots: impl IntoIterator<Item = Node>) -> Vec<Node> {
        let mut pending = vec![self.root()];
        pending.extend(roots);
        let mut reachable = HashSet::new();
        while let Some(node) = pending.pop() {
            if !reachable.insert(node) {
                continue;
            }
            let Ok(entry) = self.entry(node) else {
                continue;
            };
            pending.extend(entry.parent);
            pending.extend(entry.children.iter().copied());
        }
        let dead: Vec<_> = self
            .live
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(slot, live)| {
                **live
                    && !reachable.contains(&Node {
                        owner: self.owner,
                        slot: *slot,
                        id: self.ids[*slot],
                    })
            })
            .map(|(slot, _)| Node {
                owner: self.owner,
                slot,
                id: self.ids[slot],
            })
            .collect();
        if dead.is_empty() {
            self.report(0);
            return dead;
        }
        for node in &dead {
            self.nodes[node.slot] = Entry {
                kind: Kind::Text(String::new()),
                attributes: BTreeMap::new(),
                parent: None,
                children: Vec::new(),
            };
            self.live[node.slot] = false;
            self.vacant.push(node.slot);
        }
        self.pictures.retain(|node, _| reachable.contains(node));
        self.editors.retain(|node, _| reachable.contains(node));
        self.parts.retain(|node, _| reachable.contains(node));
        if self.focused.is_some_and(|node| !reachable.contains(&node)) {
            self.focused = None;
        }
        self.invalidate();
        self.report(dead.len());
        dead
    }

    fn report(&mut self, released: usize) {
        self.collection = Collection {
            live: self.live.iter().filter(|live| **live).count(),
            released,
            capacity: self.nodes.len(),
            vacant: self.vacant.len(),
            allocations: self.next,
        };
    }
}
