use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub limit: usize,
    pub native: bool,
    pub cpu: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            limit: 100_000,
            native: false,
            cpu: false,
        }
    }
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Vertex {
    pub name: String,
    pub incoming: Vec<usize>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Kind {
    Receive { captured: u64, thread: u64 },
    Scope,
    Graph { vertices: Vec<Vertex> },
    Node { index: usize },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    Success,
    Failure,
    Abandoned,
    Panicked,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Record {
    pub id: u64,
    pub parent: Option<u64>,
    #[serde(default)]
    pub enclosing: Option<u64>,
    pub name: String,
    pub kind: Kind,
    pub thread: u64,
    pub native: Option<u64>,
    pub start: u64,
    pub end: u64,
    pub cpu: Option<u64>,
    pub outcome: Outcome,
    pub signpost: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Report {
    pub version: u32,
    pub unit: String,
    pub clock: String,
    pub process: u32,
    pub session: u64,
    pub options: Options,
    pub stopped: bool,
    pub open: usize,
    pub dropped: usize,
    pub suppression: String,
    pub native: bool,
    pub records: Vec<Record>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Status {
    pub session: u64,
    pub options: Options,
    pub stopped: bool,
    pub open: usize,
    pub dropped: usize,
    pub recorded: usize,
}
