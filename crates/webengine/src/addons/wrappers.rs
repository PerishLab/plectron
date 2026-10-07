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

#[cfg(test)]
mod tests {
    use super::Wrappers;
    use jsengine::{Engine, Value};
    use std::collections::BTreeMap;

    #[test]
    fn identity() {
        let mut engine = Engine::default();
        let mut foreign = Engine::default();
        let node = domengine::Document::default().root();
        let value = engine.object(BTreeMap::new()).unwrap();
        let other = foreign.object(BTreeMap::new()).unwrap();
        let mut wrappers = Wrappers::default();
        wrappers.insert(node, value.clone());
        wrappers.insert(node, value.clone());
        assert_eq!(wrappers.node(&value).unwrap(), node);
        assert_eq!(wrappers.get(&node), Some(&value));
        assert!(wrappers.node(&other).is_err());
        assert!(wrappers.node(&Value::Null).is_err());
        assert!(Wrappers::default().node(&value).is_err());
        let next = engine.object(BTreeMap::new()).unwrap();
        wrappers.insert(node, next.clone());
        assert!(wrappers.node(&value).is_err());
        assert_eq!(wrappers.node(&next).unwrap(), node);
        let mut roots = Vec::new();
        wrappers.roots(&mut |root| roots.push(root));
        assert_eq!(roots, [node]);
        let Value::Object(handle) = next else {
            unreachable!()
        };
        wrappers.sweep(&[handle]);
        assert!(wrappers.get(&node).is_none());
        assert!(wrappers.node(&Value::Object(handle)).is_err());
        roots.clear();
        wrappers.roots(&mut |root| roots.push(root));
        assert!(roots.is_empty());
    }
}
