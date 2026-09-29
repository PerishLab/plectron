use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Command {
    Latest { unit: u64 },
    Read { record: u64 },
    Wait { record: u64, timeout: u64 },
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Target {
    Task { unit: Option<u64>, job: u64 },
    Frame { unit: u64, frame: u64, scene: u64 },
}
impl Target {
    pub fn unit(&self) -> Option<u64> {
        match self {
            Self::Task { unit, .. } => *unit,
            Self::Frame { unit, .. } => Some(*unit),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum State {
    Pending,
    Complete { result: Result<String, String> },
    Cancelled { reason: String },
    Unavailable { reason: String },
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Record {
    pub id: u64,
    pub version: u64,
    pub target: Target,
    pub state: State,
    pub admitted: u64,
    pub produced: Option<u64>,
    pub visible: Option<u64>,
    pub seen: Option<u64>,
    pub drops: u64,
    pub detected: u64,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Status {
    Record { record: Record },
    Evicted { id: u64 },
    Unknown { id: u64 },
    Absent { unit: u64 },
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Reply {
    pub status: Status,
    pub version: u64,
    pub replied: u64,
    pub timedout: bool,
    pub closed: bool,
}
