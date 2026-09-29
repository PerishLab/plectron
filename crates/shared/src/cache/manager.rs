use super::protocol::Progress;
use super::{Cache, Command, Fault, Policy, Snapshot, Stats};
use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct Manager {
    registry: Arc<Mutex<Registry>>,
}
#[derive(Default)]
struct Registry {
    sequence: u64,
    entries: BTreeMap<u64, Record>,
}
struct Record {
    snapshot: Snapshot,
    clear: bool,
}
impl Manager {
    pub fn register<K: Eq + Hash, V>(
        &self,
        name: &str,
        policy: Policy,
    ) -> Result<Cache<K, V>, Fault> {
        let mut registry = self.registry.lock().expect("cache registry poisoned");
        if name.is_empty() || name.len() > 64 || registry.entries.len() >= 64 {
            return Err(Fault("cache registration budget exhausted"));
        }
        let id = registry
            .sequence
            .checked_add(1)
            .ok_or(Fault("cache ids exhausted"))?;
        registry.sequence = id;
        registry.entries.insert(
            id,
            Record {
                snapshot: Snapshot {
                    id,
                    name: name.into(),
                    target: policy,
                    active: policy,
                    requested: 0,
                    applied: 0,
                    closing: false,
                    closed: false,
                    stats: Stats::default(),
                },
                clear: false,
            },
        );
        Ok(Cache::new(self.clone(), id, policy))
    }

    pub fn command(&self, id: u64, command: Command) -> Result<u64, Fault> {
        let mut registry = self.registry.lock().expect("cache registry poisoned");
        let record = registry
            .entries
            .get_mut(&id)
            .ok_or(Fault("unknown cache"))?;
        let snapshot = &mut record.snapshot;
        if snapshot.closing {
            return Err(Fault("cache is closing"));
        }
        let generation = snapshot
            .requested
            .checked_add(1)
            .ok_or(Fault("cache generations exhausted"))?;
        match command {
            Command::Configure { policy } => snapshot.target = policy,
            Command::Clear => record.clear = true,
            Command::Destroy => snapshot.closing = true,
        }
        snapshot.requested = generation;
        Ok(generation)
    }

    pub fn inspect(&self) -> Vec<Snapshot> {
        self.registry
            .lock()
            .expect("cache registry poisoned")
            .entries
            .values()
            .map(|record| record.snapshot.clone())
            .collect()
    }

    pub(super) fn pending(&self, id: u64) -> (Snapshot, bool) {
        let mut registry = self.registry.lock().expect("cache registry poisoned");
        let record = registry
            .entries
            .get_mut(&id)
            .expect("owned cache registration");
        (record.snapshot.clone(), std::mem::take(&mut record.clear))
    }

    pub(super) fn publish(&self, id: u64, progress: Progress) {
        let mut registry = self.registry.lock().expect("cache registry poisoned");
        let record = registry
            .entries
            .get_mut(&id)
            .expect("owned cache registration");
        record.snapshot.active = progress.policy;
        record.snapshot.applied = progress.generation;
        record.snapshot.closed = progress.closed;
        record.snapshot.stats = progress.stats;
    }

    pub(super) fn unregister(&self, id: u64) {
        self.registry
            .lock()
            .expect("cache registry poisoned")
            .entries
            .remove(&id);
    }
}
