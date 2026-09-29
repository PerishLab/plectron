use crate::snapshot::{Editor, Node};
use domengine::{
    Document, Kind,
    change::{Fact, Version},
};
use std::sync::Arc;

#[derive(Default)]
pub(super) struct Nodes {
    version: Option<Version>,
    slots: Vec<Option<Arc<Node>>>,
    view: Arc<[Arc<Node>]>,
}
impl Nodes {
    pub fn publish(&mut self, document: &Document) -> Arc<[Arc<Node>]> {
        if self.version == Some(document.version()) {
            return self.view.clone();
        }
        match self.version.and_then(|version| document.changes(version)) {
            None => self.slots.clear(),
            Some(changes) => {
                for change in changes {
                    self.invalidate(change.fact);
                }
            }
        }
        let order = document.walk();
        let length = order.iter().map(|node| node.slot() + 1).max().unwrap_or(0);
        let mut slots = vec![None; length];
        let mut view = Vec::with_capacity(order.len());
        for node in order {
            let cached = self.slots.get_mut(node.slot()).and_then(Option::take);
            let value = cached.unwrap_or_else(|| Arc::new(capture(document, node)));
            slots[node.slot()] = Some(value.clone());
            view.push(value);
        }
        self.slots = slots;
        self.view = view.into();
        self.version = Some(document.version());
        self.view.clone()
    }
    fn invalidate(&mut self, fact: Fact) {
        let node = match fact {
            Fact::Image { node }
            | Fact::Attribute { node, .. }
            | Fact::Text { node }
            | Fact::Attached { node, .. }
            | Fact::Value { node }
            | Fact::Selection { node }
            | Fact::Composition { node } => node,
            Fact::Focus { .. } => return,
        };
        if let Some(slot) = self.slots.get_mut(node.slot()) {
            *slot = None;
        }
    }
}
fn capture(document: &Document, node: domengine::Node) -> Node {
    let entry = document.entry(node).expect("walk owns nodes");
    let (kind, content) = match &entry.kind {
        Kind::Element(tag) => ("element", tag),
        Kind::Text(text) => ("text", text),
    };
    Node {
        id: node.id(),
        parent: entry.parent.map(|node| node.id()),
        kind: kind.into(),
        content: content.clone(),
        attributes: entry.attributes.clone(),
        editor: document.buffer(node).ok().map(|buffer| Editor {
            text: buffer.text().into(),
            selection: buffer.selection(),
            composition: buffer.composition().cloned(),
            caret: document.cursor(node).map(|caret| caret.id()),
        }),
    }
}
