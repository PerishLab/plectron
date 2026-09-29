use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Images {
    pub scans: u64,
    pub visited: u64,
    pub changes: u64,
    pub candidates: u64,
    pub fetches: u64,
    pub decoded: u64,
    pub bindings: u64,
}
