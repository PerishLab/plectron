use super::{Element, Node};
use std::{collections::BTreeMap, sync::Arc};

type Index<'a> = BTreeMap<Node, &'a Arc<Element>>;
pub(super) struct Lineage<'a>(Index<'a>);

impl<'a> Lineage<'a> {
    pub fn new(root: Option<&'a Arc<Element>>) -> Self {
        let mut index = BTreeMap::new();
        if let Some(root) = root {
            gather(root, &mut index);
        }
        Self(index)
    }
    pub fn find(&self, node: Node) -> Option<&'a Arc<Element>> {
        self.0.get(&node).copied()
    }
}

fn gather<'a>(element: &'a Arc<Element>, index: &mut Index<'a>) {
    index.insert(element.node, element);
    for child in &element.children {
        if child.node != element.node {
            gather(child, index);
        }
    }
}

pub(super) struct Children<'a> {
    remaining: &'a [Arc<Element>],
    index: Option<Index<'a>>,
}
impl<'a> Children<'a> {
    pub fn new(previous: Option<&'a Arc<Element>>) -> Self {
        Self {
            remaining: previous.map_or(&[], |old| &old.children),
            index: None,
        }
    }
    pub fn find(&mut self, node: Node) -> Option<&'a Arc<Element>> {
        if let Some(index) = &self.index {
            return index.get(&node).copied();
        }
        while let Some((first, rest)) = self.remaining.split_first() {
            if first.key.editing {
                self.remaining = rest;
                continue;
            }
            if first.node == node {
                self.remaining = rest;
                return Some(first);
            }
            let index = self.index.insert(
                self.remaining
                    .iter()
                    .filter(|child| !child.key.editing)
                    .map(|child| (child.node, child))
                    .collect(),
            );
            return index.get(&node).copied();
        }
        None
    }
}
