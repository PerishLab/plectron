use crate::shared::{Evaluation, Fault, Frame, Item, Mapping, Output, Rasterization};
use serde::Deserialize;
use shared::cache::{Cache, Command, Cost, Manager, Policy, Snapshot};
use std::{
    collections::BTreeMap,
    hash::{Hash, Hasher},
    sync::Arc,
};

const POLICY: Policy = Policy {
    read: false,
    write: false,
    entries: 2048,
    bytes: 16_777_216,
};

#[derive(Clone, Copy, Default)]
pub(crate) struct Charge {
    pub rectangles: usize,
    pub samples: usize,
    pub texels: usize,
    pub mappings: usize,
}
#[derive(PartialEq, Eq)]
struct Context {
    offsets: BTreeMap<u64, usize>,
    output: Output,
    evaluation: Evaluation,
    rasterization: Rasterization,
}
struct Key {
    items: Arc<[Item]>,
    mapping: Mapping,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.items, &other.items) && self.mapping == other.mapping
    }
}
impl Eq for Key {}
impl Hash for Key {
    fn hash<H: Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.items).hash(state);
        self.mapping.hash(state);
    }
}
pub(super) struct Groups {
    cache: Cache<Key, Charge>,
    manager: Manager,
    config: config::Store,
    setting: Option<config::Value>,
    context: Option<Context>,
    pending: Vec<(Key, Charge)>,
}
impl Groups {
    pub fn new(config: config::Store) -> Self {
        let manager = Manager::default();
        Self {
            cache: manager
                .register("groups", POLICY)
                .expect("fresh group cache"),
            manager,
            config,
            setting: None,
            context: None,
            pending: Vec::new(),
        }
    }
    pub fn begin(
        &mut self,
        frame: &Frame,
        output: Output,
        modes: (Evaluation, Rasterization),
    ) -> Result<(), Fault> {
        self.pending.clear();
        let setting = self.config.get("render.groups");
        if setting != self.setting {
            let policy = setting
                .clone()
                .map(Policy::deserialize)
                .transpose()
                .map_err(|error| Fault(format!("config render.groups: {error}")))?
                .unwrap_or(POLICY);
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
            self.setting = setting;
        }
        self.cache.maintain();
        let context = Context {
            offsets: frame.visual().offsets.clone(),
            output,
            evaluation: modes.0,
            rasterization: modes.1,
        };
        if self.context.as_ref() != Some(&context) {
            self.cache.clear();
            self.context = Some(context);
        }
        Ok(())
    }
    pub fn enabled(&self) -> bool {
        self.cache.reading() || self.cache.writing()
    }
    pub fn inspect(&self) -> Snapshot {
        self.cache.publish();
        self.manager.inspect().remove(0)
    }
    pub fn command(&mut self, command: Command) -> Result<(), Fault> {
        self.manager
            .command(self.cache.id(), command)
            .map_err(|error| Fault(error.to_string()))?;
        self.cache.maintain();
        Ok(())
    }
    pub fn get(&mut self, items: &Arc<[Item]>, mapping: &Mapping) -> Option<Charge> {
        if self.cache.stats().entries == 0 {
            return None;
        }
        self.cache.get(&Key {
            items: items.clone(),
            mapping: mapping.clone(),
        })
    }
    pub fn mark(&self) -> usize {
        self.pending.len()
    }
    pub fn store(&mut self, items: &Arc<[Item]>, mapping: &Mapping, context: (Charge, usize)) {
        if !self.cache.writing() {
            return;
        }
        self.pending.truncate(context.1);
        self.pending.push((
            Key {
                items: items.clone(),
                mapping: mapping.clone(),
            },
            context.0,
        ));
    }
    pub fn finish(&mut self) {
        for (key, charge) in self.pending.drain(..) {
            if let Some(mut cost) = retained(&key.items) {
                cost.bytes += std::mem::size_of::<Key>() + std::mem::size_of::<Charge>();
                self.cache.store(key, charge, cost);
            }
        }
    }
}
fn retained(items: &[Item]) -> Option<Cost> {
    let mut cost = Cost::new(std::mem::size_of_val(items));
    for item in items {
        match item {
            Item::Group { items, .. } => cost.extend(&retained(items)?),
            Item::Draw { record, .. } => {
                cost.extend(&record.ink().cost());
                cost.bytes += std::mem::size_of::<crate::shared::Record>();
            }
            Item::Fill { .. } => (),
            Item::Text { .. } => return None,
        }
    }
    Some(cost)
}
