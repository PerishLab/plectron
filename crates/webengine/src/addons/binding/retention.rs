use super::super::{Native, Web};
use crate::Fault;
use domengine::Node;
use jsengine::{Engine, Handle, Value};

impl Web {
    pub(crate) fn roots(&self, visit: &mut dyn FnMut(Node)) {
        self.wrappers.roots(visit);
        self.scrolls.roots(visit);
        self.visuals.nodes(visit);
    }

    pub(crate) fn prune(&mut self, dead: &[Node]) {
        self.subscriptions.prune(dead);
        self.visuals.prune(dead);
        let mut animations = self.animations.lock().expect("animation state");
        for node in dead {
            animations.cancel(*node);
        }
    }

    pub(crate) fn maintain(&mut self) {
        self.validation.maintain();
    }

    pub fn slots(&self) -> usize {
        self.bindings.len()
    }

    fn trace(&self, node: Node, visit: &mut dyn FnMut(&Value)) {
        let mut pending = vec![node];
        let mut seen = std::collections::HashSet::new();
        while let Some(node) = pending.pop() {
            if !seen.insert(node) {
                continue;
            }
            if let Some(wrapper) = self.wrappers.get(&node) {
                visit(wrapper);
            }
            self.subscriptions.on(node, visit);
            if let Ok(entry) = self.document.entry(node) {
                pending.extend(entry.parent);
                pending.extend(entry.children.iter().copied());
            }
        }
    }

    fn isolate(&self, node: Node, visit: &mut dyn FnMut(&Value)) {
        if !self.document.connected(node).unwrap_or(false) {
            self.trace(node, visit);
        }
    }
}

impl jsengine::Addons for Web {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: jsengine::Invocation,
    ) -> Result<Value, jsengine::Failure> {
        let native = self
            .bindings
            .get(binding)
            .and_then(Option::as_ref)
            .ok_or_else(|| Fault("unknown Web binding".into()))?
            .clone();
        let native = if let Native::Method(method) = native {
            let node = match self.node(&invocation.receiver) {
                Ok(node) => node,
                Err(fault) => {
                    return Err(jsengine::Failure::Throw(
                        engine.exception(jsengine::Error::Type, Some(fault.to_string()))?,
                    ));
                }
            };
            method(node)
        } else {
            native
        };
        self.calls.record(&native);
        Ok(self.invoke(engine, native, invocation.arguments)?)
    }

    fn roots(&self, visit: &mut dyn FnMut(&Value)) {
        if let Some(value) = &self.prototype {
            visit(value);
        }
        if let Some(value) = &self.receiver {
            visit(value);
        }
        for value in self.rules.values() {
            visit(value);
        }
        self.fetch.roots(visit);
        self.trace(self.document.root(), visit);
        self.visuals.values(visit);
    }

    fn edges(&self, object: Handle, visit: &mut dyn FnMut(&Value)) {
        if let Ok(node) = self.wrappers.node(&Value::Object(object)) {
            self.isolate(node, visit);
        }
    }

    fn native(&self, binding: usize, visit: &mut dyn FnMut(&Value)) {
        let Some(Some(native)) = self.bindings.get(binding) else {
            return;
        };
        let node = match native {
            Native::Dispatch(node)
            | Native::Focus(node)
            | Native::Value(node)
            | Native::Assign(node)
            | Native::Caret(node)
            | Native::Select(node)
            | Native::Attribute(node)
            | Native::Children(node)
            | Native::Tag(node)
            | Native::Read(node)
            | Native::Get(node)
            | Native::Put(node)
            | Native::Append(node)
            | Native::Remove(node)
            | Native::Replace(node)
            | Native::Listen(node)
            | Native::Follow(_, node)
            | Native::Restore(_, node)
            | Native::Unscroll(_, node)
            | Native::Unvisual(_, node) => Some(*node),
            Native::Unlisten(id) => self.subscriptions.one(*id, visit),
            Native::Release(id) => {
                self.subscriptions.group(*id, &mut |node, callback| {
                    visit(callback);
                    self.isolate(node, visit);
                });
                None
            }
            _ => None,
        };
        if let Some(node) = node {
            self.isolate(node, visit);
        }
    }

    fn sweep(&mut self, dead: &[Handle], live: &dyn Fn(usize) -> bool) {
        self.wrappers.sweep(dead);
        self.subscriptions.sweep(dead);
        for handle in dead {
            if let Some(scope) = self.scopes.remove(&handle.id()) {
                self.owners.remove(&scope);
            }
        }
        let controllers = self
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(id, binding)| match binding {
                Some(
                    Native::Follow(lease, _)
                    | Native::Restore(lease, _)
                    | Native::Unscroll(lease, _),
                ) if live(id) => Some(*lease),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>();
        let observers = self
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(id, binding)| match binding {
                Some(Native::Unvisual(lease, _)) if live(id) => Some(*lease),
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>();
        for (id, binding) in self.bindings.iter_mut().enumerate() {
            if binding.is_some() && !live(id) {
                if let Some(
                    Native::Follow(lease, _)
                    | Native::Restore(lease, _)
                    | Native::Unscroll(lease, _),
                ) = binding
                    && !controllers.contains(lease)
                {
                    self.scrolls.release(*lease);
                }
                if let Some(Native::Unvisual(lease, _)) = binding
                    && !observers.contains(lease)
                {
                    self.visuals.release(*lease);
                }
                *binding = None;
                self.vacant.push(id);
            }
        }
    }
}
