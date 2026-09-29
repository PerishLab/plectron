use super::{Document, Node};
use crate::Fault;

impl Document {
    pub(super) fn attachment(&self, parent: Node, child: Node) -> Result<(usize, bool), Fault> {
        let mut depth = 0;
        let mut current = Some(parent);
        while let Some(node) = current {
            if node == child {
                return Err(Fault("node insertion would create a cycle".into()));
            }
            depth += 1;
            if node == self.root() {
                return Ok((depth, true));
            }
            current = self.entry(node)?.parent;
        }
        Ok((depth, false))
    }

    pub fn path(&self, node: Node) -> Result<Vec<Node>, Fault> {
        let mut path = vec![node];
        let mut cursor = self.entry(node)?.parent;
        while let Some(parent) = cursor {
            path.push(parent);
            cursor = self.entry(parent)?.parent;
        }
        Ok(path)
    }

    pub fn connected(&self, node: Node) -> Result<bool, Fault> {
        let mut current = node;
        loop {
            let entry = self.entry(current)?;
            if current == self.root() {
                return Ok(true);
            }
            let Some(parent) = entry.parent else {
                return Ok(false);
            };
            current = parent;
        }
    }

    pub fn find(&self, id: &str) -> Option<Node> {
        self.walk().into_iter().find(|node| {
            self.nodes[node.slot]
                .attributes
                .get("id")
                .is_some_and(|value| value == id)
        })
    }

    pub fn walk(&self) -> Vec<Node> {
        let mut stack = vec![self.root()];
        let mut nodes = Vec::new();
        while let Some(node) = stack.pop() {
            stack.extend(self.nodes[node.slot].children.iter().rev());
            nodes.push(node);
        }
        nodes
    }

    pub(super) fn height(&self, node: Node) -> Result<usize, Fault> {
        let mut height = 0;
        for child in &self.entry(node)?.children {
            height = height.max(self.height(*child)?);
        }
        Ok(height + 1)
    }
}
