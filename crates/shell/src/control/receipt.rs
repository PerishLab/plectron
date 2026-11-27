use super::json::Value;
use std::collections::{BTreeMap, VecDeque};

pub const CAPACITY: usize = 256;
pub const TOKEN: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Accepted,
    Running,
    Completed,
    Rejected,
    Unknown,
}

impl Status {
    pub const fn terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Rejected | Self::Unknown)
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Rejected => "rejected",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Update {
    pub current: u64,
    pub total: Option<u64>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
    pub command: String,
    pub target: String,
    pub status: Status,
    pub value: Option<Value>,
    pub error: Option<String>,
    pub progress: Option<Update>,
}

impl Receipt {
    pub fn accepted(command: impl Into<String>, target: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            target: target.into(),
            status: Status::Accepted,
            value: None,
            error: None,
            progress: None,
        }
    }

    pub fn rejected(
        command: impl Into<String>,
        target: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            command: command.into(),
            target: target.into(),
            status: Status::Rejected,
            value: None,
            error: Some(reason.into()),
            progress: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Admission {
    Execute(Receipt),
    Duplicate(Receipt),
    Refused(Receipt),
}

pub struct Ledger {
    capacity: usize,
    order: VecDeque<String>,
    entries: BTreeMap<String, Receipt>,
}

impl Default for Ledger {
    fn default() -> Self {
        Self::new(CAPACITY)
    }
}

impl Ledger {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            order: VecDeque::new(),
            entries: BTreeMap::new(),
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn query(&self, id: &str) -> Option<&Receipt> {
        self.entries.get(id)
    }

    pub fn admit(&mut self, id: &str, target: &str) -> Result<Admission, String> {
        validate(id, "command_id")?;
        validate(target, "command")?;
        if let Some(existing) = self.entries.get(id) {
            return Ok(Admission::Duplicate(existing.clone()));
        }
        if self.entries.len() >= self.capacity && !self.evict() {
            return Ok(Admission::Refused(Receipt::rejected(
                id,
                target,
                "receipt ledger capacity exhausted",
            )));
        }
        let receipt = Receipt::accepted(id, target);
        self.order.push_back(id.into());
        self.entries.insert(id.into(), receipt.clone());
        Ok(Admission::Execute(receipt))
    }

    pub fn start(&mut self, id: &str) -> Result<Receipt, String> {
        let entry = self.mutable(id)?;
        if entry.status != Status::Accepted {
            return Err("receipt can only start from accepted state".into());
        }
        entry.status = Status::Running;
        Ok(entry.clone())
    }

    pub fn progress(&mut self, id: &str, update: Update) -> Result<Receipt, String> {
        let entry = self.mutable(id)?;
        entry.status = Status::Running;
        entry.progress = Some(update);
        Ok(entry.clone())
    }

    pub fn complete(&mut self, id: &str, value: Value) -> Result<Receipt, String> {
        let entry = self.mutable(id)?;
        entry.status = Status::Completed;
        entry.value = Some(value);
        entry.error = None;
        Ok(entry.clone())
    }

    pub fn reject(&mut self, id: &str, reason: String) -> Result<Receipt, String> {
        let entry = self.mutable(id)?;
        entry.status = Status::Rejected;
        entry.value = None;
        entry.error = Some(reason);
        Ok(entry.clone())
    }

    pub fn uncertain(&mut self, id: &str, reason: String) -> Result<Receipt, String> {
        let entry = self.mutable(id)?;
        entry.status = Status::Unknown;
        entry.value = None;
        entry.error = Some(reason);
        Ok(entry.clone())
    }

    fn mutable(&mut self, id: &str) -> Result<&mut Receipt, String> {
        let entry = self
            .entries
            .get_mut(id)
            .ok_or_else(|| "command_id not found".to_string())?;
        if entry.status.terminal() {
            return Err("terminal receipt is immutable".into());
        }
        Ok(entry)
    }

    fn evict(&mut self) -> bool {
        let position = self.order.iter().position(|id| {
            self.entries
                .get(id)
                .is_some_and(|entry| matches!(entry.status, Status::Completed | Status::Rejected))
        });
        let Some(index) = position else {
            return false;
        };
        if let Some(id) = self.order.remove(index) {
            self.entries.remove(&id);
            return true;
        }
        false
    }
}

pub(crate) fn validate(token: &str, field: &str) -> Result<(), String> {
    if token.is_empty() || token.len() > TOKEN {
        return Err(format!("{field} must be between 1 and {TOKEN} bytes"));
    }
    if token.chars().any(char::is_control) {
        return Err(format!("{field} contains control characters"));
    }
    Ok(())
}
