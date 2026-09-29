use crate::Fault;
use crate::layout::input::Key;
use crate::layout::text::operation::{Access, Node};
use crate::layout::text::paragraph::Paragraph;
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy};
use std::sync::Arc;

type Size = [usize; 2];
pub(super) type Identity = (Arc<Key>, Vec<Size>);
const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};

pub(in crate::layout) struct Paragraphs {
    cache: Cache<Identity, Entry>,
    config: config::Store,
    policy: Option<config::Value>,
    routing: bool,
}
#[derive(Clone)]
struct Entry {
    paragraph: Paragraph,
    node: Node,
}
impl Paragraphs {
    pub fn new(manager: &Manager, config: config::Store) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            cache: manager.register("paragraphs", POLICY)?,
            config,
            policy: None,
            routing: true,
        })
    }
    pub fn configure(&mut self) -> Result<(), Fault> {
        self.routing = self
            .config
            .get("layout.routing")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.routing: {error}")))?
            .unwrap_or(true);
        let value = self.config.get("layout.paragraphs");
        if self.policy != value {
            let policy = value
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config layout.paragraphs: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.policy = value;
        }
        Ok(())
    }
    pub fn find(&mut self, key: &Identity) -> Option<(Paragraph, Node)> {
        self.cache
            .lookup(key)
            .map(|entry| (entry.paragraph.clone(), entry.node.clone()))
    }
    pub fn store(&mut self, key: Identity, paragraph: &Paragraph) -> Node {
        let node = Node::new(paragraph);
        if self.cache.writing() {
            let mut cost = paragraph.cost();
            cost.bytes += key.0.weight
                + std::mem::size_of_val(key.1.as_slice())
                + std::mem::size_of::<Entry>()
                + node.cost();
            self.cache.store(
                key,
                Entry {
                    paragraph: paragraph.clone(),
                    node: node.clone(),
                },
                cost,
            );
        }
        node
    }
    pub fn access(&self) -> Access {
        Access {
            reuse: self.routing && self.cache.reading(),
            remember: self.routing && self.cache.writing(),
        }
    }
    pub fn clear(&mut self) {
        self.cache.clear();
    }
    pub fn maintain(&mut self) {
        self.cache.maintain();
    }
    pub fn close(&mut self) {
        self.cache.close();
    }
    pub fn publish(&self) {
        self.cache.publish();
    }
    pub fn enable(&mut self, enabled: bool) {
        let mut policy = self.cache.policy();
        policy.read = enabled;
        policy.write = enabled;
        let _ = self.cache.configure(policy);
    }
}
