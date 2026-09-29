use crate::{Document, Fault};
use serde::{Deserialize, Serialize};

pub(super) const LIMIT: usize = 8192;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Retention {
    pub limit: usize,
    pub entries: usize,
    pub bytes: usize,
    pub floor: u64,
}

impl Document {
    pub fn configure(&mut self, config: &config::Store) -> Result<(), Fault> {
        let limit = config
            .get("dom.history")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config dom.history: {error}")))?
            .unwrap_or(LIMIT);
        if limit != self.journal.limit {
            self.journal.limit = limit;
            let discard = self.journal.entries.len().saturating_sub(limit);
            self.journal.entries.drain(..discard);
            self.journal.entries.shrink_to_fit();
        }
        Ok(())
    }

    pub fn retention(&self) -> Retention {
        Retention {
            limit: self.journal.limit,
            entries: self.journal.entries.len(),
            bytes: self.journal.entries.capacity() * std::mem::size_of::<super::Change>(),
            floor: self
                .journal
                .entries
                .front()
                .map_or(self.revision(), |change| change.revision - 1),
        }
    }
}
