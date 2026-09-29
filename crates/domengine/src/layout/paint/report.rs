use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Painting {
    #[serde(default)]
    pub identified: usize,
    pub elapsed: u64,
    pub revision: u64,
    pub enabled: bool,
    pub requests: usize,
    pub reused: usize,
    #[serde(default)]
    pub relocated: usize,
    #[serde(default)]
    pub placed: usize,
    #[serde(default)]
    pub routed: usize,
    #[serde(default)]
    pub checked: usize,
    #[serde(default)]
    pub built: usize,
    pub hits: usize,
    pub recorded: usize,
    pub glyphs: usize,
    pub groups: usize,
    pub failure: Option<String>,
}
