use crate::shared::Fault;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Target {
    pub(crate) owner: u64,
    pub(crate) slot: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Submission {
    pub(crate) owner: u64,
    pub(crate) slot: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum State {
    Pending,
    Complete,
    Failed(Fault),
}
