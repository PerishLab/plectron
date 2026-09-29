use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct Timing {
    pub topology: u64,
    pub planning: u64,
    pub facts: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Update {
    #[serde(default)]
    pub history: crate::change::Retention,
    pub elapsed: u64,
    #[serde(default)]
    pub timing: Timing,
    pub enabled: bool,
    pub revision: u64,
    pub committed: Option<u64>,
    pub changes: usize,
    pub affected: usize,
    pub ancestry: usize,
    pub visited: usize,
    pub rebuilt: usize,
    pub reused: usize,
    pub skipped: usize,
    pub styled: usize,
    pub projected: usize,
    pub nodes: usize,
    pub reset: Option<String>,
    pub failure: Option<String>,
}
