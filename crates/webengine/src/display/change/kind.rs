use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work {
    pub updates: u64,
    pub rebuilds: u64,
    pub incremental: u64,
    pub spaces: u64,
    pub positions: u64,
    pub elapsed: u64,
}

impl Work {
    pub fn since(self, previous: Self) -> Self {
        Self {
            updates: self.updates - previous.updates,
            rebuilds: self.rebuilds - previous.rebuilds,
            incremental: self.incremental - previous.incremental,
            spaces: self.spaces - previous.spaces,
            positions: self.positions - previous.positions,
            elapsed: self.elapsed - previous.elapsed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cause {
    User,
    Program,
    Policy,
    Restore,
    Anchor,
    Layout,
}

impl Cause {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Program => "program",
            Self::Policy => "policy",
            Self::Restore => "restore",
            Self::Anchor => "anchor",
            Self::Layout => "layout",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Boundary {
    Start,
    Middle,
    End,
}

impl Boundary {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Middle => "middle",
            Self::End => "end",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub target: usize,
    pub inset: i64,
    pub fallbacks: Vec<Position>,
}
