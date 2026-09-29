use super::{Cache, Cost};
use std::hash::Hash;

impl<K: Eq + Hash, V> Cache<K, V> {
    pub fn replace(&mut self, key: &K, value: V, cost: Cost) -> Result<(), (V, Cost)> {
        let bytes = cost.checked();
        if !self.writing()
            || self.policy.entries == 0
            || bytes.is_none_or(|n| n > self.policy.bytes)
        {
            return Err((value, cost));
        }
        let Some(entry) = self.entries.get_mut(key) else {
            return Err((value, cost));
        };
        self.ledger.release(&entry.cost);
        self.ledger.publish(&mut self.stats);
        if self.ledger.extra(&cost) > self.policy.bytes - self.stats.bytes {
            self.ledger.retain(&entry.cost);
            self.ledger.publish(&mut self.stats);
            return Err((value, cost));
        }
        self.ledger.retain(&cost);
        self.ledger.publish(&mut self.stats);
        entry.value = value;
        entry.cost = cost;
        self.order.touch(entry.slot);
        self.stats.largest = self.stats.largest.max(bytes.unwrap());
        self.stats.inserted = self.stats.inserted.saturating_add(1);
        self.stats.released = self.stats.released.saturating_add(1);
        Ok(())
    }
}
