use crate::Fault;
use std::sync::Arc;

pub struct Buffer {
    bytes: Arc<[u8]>,
    generation: u64,
    published: bool,
}
pub struct Lease {
    bytes: Arc<[u8]>,
    generation: u64,
}
impl Buffer {
    pub fn new(capacity: usize) -> Result<Self, Fault> {
        if capacity == 0 || capacity > 16_777_216 {
            return Err(Fault("buffer capacity exceeds budget".into()));
        }
        Ok(Self {
            bytes: vec![0; capacity].into(),
            generation: 0,
            published: false,
        })
    }
    pub fn writable(&mut self) -> Result<&mut [u8], Fault> {
        if self.published {
            return Err(Fault("published buffer is immutable".into()));
        }
        Arc::get_mut(&mut self.bytes).ok_or_else(|| Fault("buffer is borrowed".into()))
    }
    pub fn publish(&mut self) -> Result<u64, Fault> {
        if self.published {
            return Err(Fault("buffer is already published".into()));
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| Fault("buffer generation exhausted".into()))?;
        self.published = true;
        Ok(self.generation)
    }
    pub fn borrow(&self, generation: u64) -> Result<Lease, Fault> {
        if !self.published || generation != self.generation {
            return Err(Fault("stale buffer publication".into()));
        }
        Ok(Lease {
            bytes: self.bytes.clone(),
            generation,
        })
    }
    pub fn reclaim(&mut self, generation: u64) -> Result<(), Fault> {
        if !self.published || generation != self.generation {
            return Err(Fault("stale buffer publication".into()));
        }
        if Arc::strong_count(&self.bytes) != 1 {
            return Err(Fault("buffer is borrowed".into()));
        }
        self.published = false;
        Ok(())
    }
}
impl Lease {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
}
