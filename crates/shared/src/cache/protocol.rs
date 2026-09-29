#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Policy {
    pub read: bool,
    pub write: bool,
    pub entries: usize,
    pub bytes: usize,
}
impl Policy {
    pub fn bounded(entries: usize, bytes: usize) -> Self {
        Self {
            read: true,
            write: true,
            entries,
            bytes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "action", rename_all = "lowercase"))]
pub enum Command {
    Configure { policy: Policy },
    Clear,
    Destroy,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Stats {
    pub local: usize,
    pub shared: usize,
    pub resources: usize,
    pub entries: usize,
    pub bytes: usize,
    pub requests: u64,
    pub hits: u64,
    pub inserted: u64,
    pub omitted: u64,
    pub blocked: u64,
    pub oversized: u64,
    pub overflowed: u64,
    pub largest: usize,
    pub evicted: u64,
    pub released: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Snapshot {
    pub id: u64,
    pub name: String,
    pub target: Policy,
    pub active: Policy,
    pub requested: u64,
    pub applied: u64,
    pub closing: bool,
    pub closed: bool,
    pub stats: Stats,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault(pub &'static str);
impl std::fmt::Display for Fault {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str(self.0)
    }
}
impl std::error::Error for Fault {}

pub(super) struct Progress {
    pub policy: Policy,
    pub generation: u64,
    pub closed: bool,
    pub stats: Stats,
}
