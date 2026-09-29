use serde::Serialize;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Strategy {
    Poll,
    Platform,
    Completion,
    Both,
}
impl Strategy {
    pub fn platform(self) -> bool {
        matches!(self, Self::Platform | Self::Both)
    }
    pub fn completion(self) -> bool {
        matches!(self, Self::Completion | Self::Both)
    }
    pub fn notified(self) -> bool {
        !matches!(self, Self::Poll)
    }
}
pub const STRATEGIES: [Strategy; 4] = [
    Strategy::Poll,
    Strategy::Platform,
    Strategy::Completion,
    Strategy::Both,
];
