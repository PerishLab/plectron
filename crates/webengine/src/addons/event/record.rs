use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Capture,
    Target,
    Bubble,
}
impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Target => "target",
            Self::Bubble => "bubble",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    Host,
    Script,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Notification,
    Applied,
    Unchanged,
    Prevented,
    Retargeted,
    Failed,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delivery {
    pub subscription: u64,
    pub node: usize,
    pub phase: Phase,
    pub error: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub id: u64,
    pub parent: Option<u64>,
    pub job: Option<u64>,
    pub target: usize,
    pub kind: String,
    pub input: Option<String>,
    pub origin: Origin,
    pub path: Vec<usize>,
    pub deliveries: Vec<Delivery>,
    pub omitted: usize,
    pub errors: usize,
    pub exception: Option<String>,
    pub cancelled: Option<u64>,
    pub stopped: Option<u64>,
    pub immediate: bool,
    pub failure: Option<String>,
    pub action: Action,
}
#[derive(Debug, Clone, Copy)]
pub struct Outcome {
    pub id: u64,
    pub cancelled: bool,
}
