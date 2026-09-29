use crate::Fault;
use domengine::Node;
use jsengine::{Handle, Value};

#[derive(Default)]
pub(super) struct Wrappers {
    values: Vec<Option<Value>>,
    nodes: Vec<Option<(Handle, Node)>>,
}

impl Wrappers {
    pub fn roots(&self, visit: &mut dyn FnMut(Node)) {
        for (_, node) in self.nodes.iter().flatten() {
            visit(*node);
        }
    }

    pub fn sweep(&mut self, dead: &[Handle]) {
        for handle in dead {
            if let Some(entry) = self.nodes.get_mut(handle.id())
                && let Some((owner, node)) = entry
                && owner == handle
            {
                self.values[node.slot()] = None;
                *entry = None;
            }
        }
    }

    pub fn get(&self, node: &Node) -> Option<&Value> {
        self.values.get(node.slot()).and_then(Option::as_ref)
    }

    pub fn insert(&mut self, node: Node, value: Value) {
        let Value::Object(handle) = &value else {
            unreachable!("DOM wrappers are engine objects");
        };
        let handle = *handle;
        if self.values.len() <= node.slot() {
            self.values.resize(node.slot() + 1, None);
        }
        if let Some(Value::Object(previous)) = self.values[node.slot()].replace(value) {
            self.nodes[previous.id()] = None;
        }
        if self.nodes.len() <= handle.id() {
            self.nodes.resize(handle.id() + 1, None);
        }
        if let Some((_, previous)) = self.nodes[handle.id()].replace((handle, node))
            && previous != node
        {
            self.values[previous.slot()] = None;
        }
    }

    pub fn node(&self, value: &Value) -> Result<Node, Fault> {
        if let Value::Object(handle) = value
            && let Some(Some((owner, node))) = self.nodes.get(handle.id())
            && owner == handle
        {
            return Ok(*node);
        }
        Err(Fault("expected a node from this document".into()))
    }
}
