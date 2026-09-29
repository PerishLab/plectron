use crate::Fault;
use domengine::Node;
use jsengine::{Handle, Value};
use std::collections::{BTreeMap, HashMap, HashSet};

const SCOPES: (&str, usize) = ("events.scopes", 1024);
const SUBSCRIPTIONS: (&str, usize) = ("events.subscriptions", 4096);
const NODE: (&str, usize) = ("events.node", 64);

#[derive(Clone, Default)]
pub struct Options {
    pub capture: bool,
    pub once: bool,
    pub observe: bool,
    pub scope: u64,
}
#[derive(Clone)]
pub struct Subscription {
    pub node: Node,
    pub kind: String,
    pub callback: Value,
    pub options: Options,
}
#[derive(Default)]
pub struct Registry {
    config: config::Store,
    next: u64,
    entries: BTreeMap<u64, Subscription>,
    scopes: BTreeMap<u64, Vec<u64>>,
    nodes: HashMap<Node, usize>,
}
impl Registry {
    pub fn prune(&mut self, dead: &[Node]) {
        let dead: HashSet<_> = dead.iter().copied().collect();
        let removed: Vec<_> = self
            .entries
            .iter()
            .filter_map(|(id, entry)| dead.contains(&entry.node).then_some(*id))
            .collect();
        for id in removed {
            self.detach(id);
        }
    }

    pub fn sweep(&mut self, dead: &[Handle]) {
        let dead: HashSet<_> = dead.iter().copied().collect();
        let removed: Vec<u64> = self
            .entries
            .iter()
            .filter_map(|(id, entry)| match &entry.callback {
                Value::Object(handle) if dead.contains(handle) => Some(*id),
                _ => None,
            })
            .collect();
        for id in removed {
            self.detach(id);
        }
    }

    pub fn on(&self, node: Node, visit: &mut dyn FnMut(&Value)) {
        for entry in self.entries.values().filter(|entry| entry.node == node) {
            visit(&entry.callback);
        }
    }

    pub fn nodes(&self, kind: &str) -> Vec<Node> {
        self.entries
            .values()
            .filter(|entry| entry.kind == kind)
            .map(|entry| entry.node)
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn one(&self, id: u64, visit: &mut dyn FnMut(&Value)) -> Option<Node> {
        let entry = self.entries.get(&id)?;
        visit(&entry.callback);
        Some(entry.node)
    }

    pub fn group(&self, scope: u64, visit: &mut dyn FnMut(Node, &Value)) {
        if let Some(ids) = self.scopes.get(&scope) {
            for id in ids {
                if let Some(entry) = self.entries.get(id) {
                    visit(entry.node, &entry.callback);
                }
            }
        }
    }

    pub fn new(config: config::Store) -> Self {
        Self {
            config,
            ..Self::default()
        }
    }
    fn limit(&self, (key, fallback): (&str, usize)) -> Result<usize, Fault> {
        let Some(value) = self.config.get(key) else {
            return Ok(fallback);
        };
        value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| Fault(format!("config {key} requires a nonnegative integer")))
    }
    fn identifier(&mut self) -> Result<u64, Fault> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| Fault("subscription IDs exhausted".into()))?;
        Ok(self.next)
    }
    pub fn scope(&mut self) -> Result<u64, Fault> {
        if self.scopes.len() >= self.limit(SCOPES)? {
            return Err(Fault("subscription scope budget exhausted".into()));
        }
        let id = self.identifier()?;
        self.scopes.insert(id, Vec::new());
        Ok(id)
    }
    pub fn release(&mut self, scope: u64) -> Result<(), Fault> {
        let entries = self
            .scopes
            .remove(&scope)
            .ok_or_else(|| Fault("subscription scope is disposed".into()))?;
        for id in entries {
            self.detach(id);
        }
        Ok(())
    }
    pub fn register(&mut self, entry: Subscription) -> Result<u64, Fault> {
        if entry.options.scope != 0 && !self.scopes.contains_key(&entry.options.scope) {
            return Err(Fault("subscription scope is disposed".into()));
        }
        if self.entries.len() >= self.limit(SUBSCRIPTIONS)?
            || self.nodes.get(&entry.node).copied().unwrap_or(0) >= self.limit(NODE)?
        {
            return Err(Fault("subscription budget exhausted".into()));
        }
        let id = self.identifier()?;
        if entry.options.scope != 0 {
            self.scopes
                .get_mut(&entry.options.scope)
                .expect("validated subscription scope")
                .push(id);
        }
        *self.nodes.entry(entry.node).or_default() += 1;
        self.entries.insert(id, entry);
        Ok(id)
    }
    pub fn remove(&mut self, id: u64) -> Result<(), Fault> {
        self.detach(id)
            .map(|_| ())
            .ok_or_else(|| Fault("subscription is disposed".into()))
    }
    pub fn snapshot(&self, node: Node, kind: &str, capture: bool) -> Vec<u64> {
        self.entries
            .iter()
            .filter(|(_, entry)| {
                entry.node == node && entry.kind == kind && entry.options.capture == capture
            })
            .map(|(id, _)| *id)
            .collect()
    }
    pub fn take(&mut self, id: u64) -> Option<Subscription> {
        let entry = self.entries.get(&id)?.clone();
        if entry.options.once {
            self.detach(id);
        }
        Some(entry)
    }
    fn detach(&mut self, id: u64) -> Option<Subscription> {
        let entry = self.entries.remove(&id)?;
        if entry.options.scope != 0
            && let Some(scope) = self.scopes.get_mut(&entry.options.scope)
        {
            scope.retain(|entry| *entry != id);
        }
        let count = self
            .nodes
            .get_mut(&entry.node)
            .expect("registered node count");
        *count -= 1;
        if *count == 0 {
            self.nodes.remove(&entry.node);
        }
        Some(entry)
    }
}
