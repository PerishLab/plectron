use crate::Constraints;
use crate::layout::cache::Measurement;
use crate::layout::input::Key;
use std::sync::Arc;

const CAPACITY: usize = 16;
type Identity = (Arc<Key>, Constraints);

pub(super) struct Window {
    config: config::Store,
    capacity: usize,
    entries: Vec<(u64, Identity, Sample)>,
    seen: Vec<(u64, Constraints)>,
}

pub(super) struct Sample {
    pub geometry: Arc<Measurement>,
    pub work: usize,
    pub promote: bool,
    cost: usize,
}

impl Window {
    pub fn new(config: config::Store) -> Self {
        Self {
            config,
            capacity: CAPACITY,
            entries: Default::default(),
            seen: Default::default(),
        }
    }

    pub fn configure(&mut self) -> Result<(), crate::Fault> {
        self.capacity = self
            .config
            .get("layout.window")
            .map(serde::Deserialize::deserialize)
            .transpose()
            .map_err(|error| crate::Fault(format!("config layout.window: {error}")))?
            .unwrap_or(CAPACITY);
        Ok(())
    }

    pub fn find(&mut self, key: &Identity) -> Option<Sample> {
        let fingerprint = key.0.fingerprint();
        let index = self.entries.iter().rposition(|(digest, candidate, _)| {
            *digest == fingerprint && candidate.0 == key.0 && candidate.1 == key.1
        })?;
        let sample = &self.entries[index].2;
        if sample.promote {
            Some(self.entries.swap_remove(index).2)
        } else {
            Some(Sample {
                geometry: sample.geometry.clone(),
                work: sample.work,
                promote: false,
                cost: sample.cost,
            })
        }
    }

    pub fn store(&mut self, key: Identity, geometry: Arc<Measurement>, work: usize, promote: bool) {
        if self.entries.len() < self.capacity {
            let cost = key.0.weight + geometry.cost().bytes;
            self.entries.push((
                key.0.fingerprint(),
                key,
                Sample {
                    geometry,
                    work,
                    promote,
                    cost,
                },
            ));
        }
    }

    pub fn recurring(&mut self, key: &Identity) -> bool {
        if self.capacity == 0 {
            return false;
        }
        let candidate = (key.0.fingerprint(), key.1);
        if self.seen.contains(&candidate) {
            return true;
        }
        if self.seen.len() < self.capacity.saturating_mul(4) {
            self.seen.push(candidate);
        }
        false
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.seen.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.2.cost).sum::<usize>()
            + self.seen.len() * std::mem::size_of::<(u64, Constraints)>()
    }
}
