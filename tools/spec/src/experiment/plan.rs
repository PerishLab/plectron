use super::Rules;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub version: u32,
    pub entries: Vec<Entry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub id: String,
    pub scenario: String,
    pub size: String,
    pub backend: String,
    pub runs: usize,
    #[serde(default)]
    pub config: config::Entries,
    pub baseline: Option<String>,
    #[serde(default)]
    pub rules: Rules,
}
impl Plan {
    pub fn read(path: &std::path::Path) -> crate::Result<Self> {
        let plan: Self = serde_json::from_value(super::storage::File(path).read()?)
            .map_err(|e| e.to_string())?;
        plan.validate()?;
        Ok(plan)
    }
    pub fn validate(&self) -> crate::Result<()> {
        if self.version != 1 || self.entries.is_empty() {
            return Err("experiment needs version 1 and nonempty entries".into());
        }
        let mut ids = BTreeSet::new();
        for entry in &self.entries {
            if !name(&entry.id) || !ids.insert(&entry.id) || entry.runs == 0 {
                return Err("invalid or duplicate experiment identity or runs".into());
            }
            if entry.config.contains_key("scenario.runs") {
                return Err("experiment repetitions belong in runs".into());
            }
        }
        Ok(())
    }
}
pub(super) fn name(name: &str) -> bool {
    !name.is_empty()
        && name != "manifest"
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
}
