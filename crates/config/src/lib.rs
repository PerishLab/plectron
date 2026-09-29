use serde::{Deserialize, Serialize};
pub use serde_json::Value;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

pub type Entries = BTreeMap<String, Value>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Command {
    Inspect {},
    Set { key: String, value: Value },
    Remove { key: String },
}

#[derive(Clone, Default)]
pub struct Store {
    entries: Arc<RwLock<Entries>>,
}

impl Store {
    pub fn get(&self, key: &str) -> Option<Value> {
        self.entries.read().expect("config read").get(key).cloned()
    }

    pub fn set(&self, key: &str, value: Value) {
        self.entries
            .write()
            .expect("config write")
            .insert(key.into(), value);
    }

    pub fn remove(&self, key: &str) {
        self.entries.write().expect("config write").remove(key);
    }

    pub fn inspect(&self) -> Entries {
        self.entries.read().expect("config read").clone()
    }

    pub fn command(&self, command: Command) -> Entries {
        match command {
            Command::Inspect {} => (),
            Command::Set { key, value } => self.set(&key, value),
            Command::Remove { key } => self.remove(&key),
        }
        self.inspect()
    }
}
