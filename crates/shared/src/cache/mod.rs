mod cost;
pub use cost::{Cost, Resource};
mod manager;
mod order;
mod protocol;
mod replacement;
pub use manager::Manager;
pub use protocol::{Command, Fault, Policy, Snapshot, Stats};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

type Table<K, V> = HashMap<Arc<K>, Entry<V>, ahash::RandomState>;

pub struct Cache<K, V> {
    manager: Manager,
    id: u64,
    policy: Policy,
    applied: u64,
    closed: bool,
    entries: Table<K, V>,
    order: order::Order<K>,
    stats: Stats,
    ledger: cost::Ledger,
}
struct Entry<V> {
    value: V,
    cost: Cost,
    slot: usize,
}
impl<K: Eq + Hash, V> Cache<K, V> {
    fn new(manager: Manager, id: u64, policy: Policy) -> Self {
        Self {
            manager,
            id,
            policy,
            applied: 0,
            closed: false,
            entries: HashMap::default(),
            order: order::Order::new(),
            stats: Stats::default(),
            ledger: cost::Ledger::default(),
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn policy(&self) -> Policy {
        self.policy
    }
    pub fn reading(&self) -> bool {
        !self.closed && self.policy.read
    }
    pub fn writing(&self) -> bool {
        !self.closed && self.policy.write
    }
    pub fn stats(&self) -> Stats {
        self.stats.clone()
    }
    pub fn get(&mut self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        self.lookup(key).cloned()
    }
    pub fn lookup(&mut self, key: &K) -> Option<&V> {
        self.stats.requests = self.stats.requests.saturating_add(1);
        if !self.reading() {
            return None;
        }
        let entry = self.entries.get_mut(key)?;
        self.order.touch(entry.slot);
        self.stats.hits = self.stats.hits.saturating_add(1);
        Some(&entry.value)
    }
    pub fn insert(&mut self, key: K, value: V, bytes: usize) -> bool {
        self.store(key, value, Cost::new(bytes))
    }
    pub fn store(&mut self, key: K, value: V, cost: Cost) -> bool {
        if !self.accept(&cost) {
            return false;
        }
        self.put(Arc::new(key), value, cost)
    }
    pub fn retain(&mut self, key: Arc<K>, value: V, cost: Cost) -> bool {
        if !self.accept(&cost) {
            return false;
        }
        self.put(key, value, cost)
    }
    fn put(&mut self, key: Arc<K>, value: V, cost: Cost) -> bool {
        self.remove(key.as_ref());
        while self.entries.len() >= self.policy.entries
            || self.ledger.extra(&cost) > self.policy.bytes - self.stats.bytes
        {
            self.evict();
        }
        let slot = self.order.insert(key.clone());
        self.ledger.retain(&cost);
        self.entries.insert(key, Entry { value, cost, slot });
        self.ledger.publish(&mut self.stats);
        self.stats.entries = self.entries.len();
        self.stats.inserted = self.stats.inserted.saturating_add(1);
        true
    }
    pub fn remove(&mut self, key: &K) -> Option<V> {
        let entry = self.entries.remove(key)?;
        self.order.remove(entry.slot);
        self.ledger.release(&entry.cost);
        self.ledger.publish(&mut self.stats);
        self.stats.entries = self.entries.len();
        self.stats.released = self.stats.released.saturating_add(1);
        Some(entry.value)
    }
    pub fn clear(&mut self) {
        self.stats.released = self
            .stats
            .released
            .saturating_add(self.entries.len() as u64);
        self.entries.clear();
        self.order = order::Order::new();
        self.stats.entries = 0;
        self.ledger = cost::Ledger::default();
        self.ledger.publish(&mut self.stats);
    }
    pub fn maintain(&mut self) {
        let (snapshot, clear) = self.manager.pending(self.id);
        self.policy = snapshot.target;
        self.applied = snapshot.requested;
        self.closed = snapshot.closing;
        if clear || self.closed {
            self.clear();
        }
        while self.entries.len() > self.policy.entries || self.stats.bytes > self.policy.bytes {
            self.evict();
        }
        self.publish();
    }
    pub fn publish(&self) {
        self.manager.publish(
            self.id,
            protocol::Progress {
                policy: self.policy,
                generation: self.applied,
                closed: self.closed,
                stats: self.stats.clone(),
            },
        );
    }

    pub fn configure(&mut self, policy: Policy) -> Result<(), Fault> {
        self.manager
            .command(self.id, Command::Configure { policy })?;
        self.maintain();
        Ok(())
    }
    pub fn close(&mut self) {
        if !self.closed {
            let _ = self.manager.command(self.id, Command::Destroy);
            self.maintain();
        }
    }
    fn evict(&mut self) {
        let key = self.order.first().expect("populated cache");
        self.remove(key.as_ref());
        self.stats.evicted = self.stats.evicted.saturating_add(1);
    }
    fn accept(&mut self, cost: &Cost) -> bool {
        let counter = if !self.writing() || self.policy.entries == 0 {
            &mut self.stats.blocked
        } else if let Some(bytes) = cost.checked() {
            self.stats.largest = self.stats.largest.max(bytes);
            if bytes <= self.policy.bytes {
                return true;
            }
            &mut self.stats.oversized
        } else {
            &mut self.stats.overflowed
        };
        *counter = counter.saturating_add(1);
        self.stats.omitted = self.stats.omitted.saturating_add(1);
        false
    }
}
impl<K, V> Drop for Cache<K, V> {
    fn drop(&mut self) {
        self.manager.unregister(self.id);
    }
}
