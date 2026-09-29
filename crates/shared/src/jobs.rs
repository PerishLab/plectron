use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Closed,
    Identifier,
    Unknown,
    Transition,
}
impl std::fmt::Display for Fault {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Closed => "job ledger closed",
            Self::Identifier => "job identifier must increase",
            Self::Unknown => "unknown job",
            Self::Transition => "invalid job transition",
        })
    }
}
impl std::error::Error for Fault {}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Running,
    Held,
    Complete,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(tag = "phase", rename_all = "lowercase"))]
pub enum State {
    Queued,
    Running,
    Held,
    Complete { result: Result<String, String> },
    Cancelled { reason: String },
}
impl State {
    pub fn phase(&self) -> Phase {
        match self {
            Self::Queued => Phase::Queued,
            Self::Running => Phase::Running,
            Self::Held => Phase::Held,
            Self::Complete { .. } => Phase::Complete,
            Self::Cancelled { .. } => Phase::Cancelled,
        }
    }
    pub fn terminal(&self) -> bool {
        matches!(self, Self::Complete { .. } | Self::Cancelled { .. })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Job<K> {
    pub id: u64,
    pub kind: K,
    #[cfg_attr(feature = "serde", serde(skip))]
    pub produced: Option<std::time::Instant>,
    #[cfg_attr(feature = "serde", serde(flatten))]
    pub state: State,
}
impl<K> Job<K> {
    pub fn phase(&self) -> Phase {
        self.state.phase()
    }
    pub fn result(&self) -> Option<&Result<String, String>> {
        match &self.state {
            State::Complete { result } => Some(result),
            _ => None,
        }
    }
}
pub struct Jobs<K> {
    records: BTreeMap<u64, Job<K>>,
    latest: u64,
    limit: usize,
    closed: bool,
}
impl<K> Jobs<K> {
    pub fn new(limit: usize) -> Self {
        Self {
            records: BTreeMap::new(),
            latest: 0,
            limit,
            closed: false,
        }
    }
    pub fn admit(&mut self, id: u64, kind: K) -> Result<(), Fault> {
        if self.closed {
            return Err(Fault::Closed);
        }
        if id <= self.latest {
            return Err(Fault::Identifier);
        }
        self.latest = id;
        self.records.insert(
            id,
            Job {
                id,
                kind,
                produced: None,
                state: State::Queued,
            },
        );
        self.trim();
        Ok(())
    }
    pub fn start(&mut self, id: u64) -> Result<bool, Fault> {
        self.transition(id, Phase::Queued, State::Running)
    }
    pub fn hold(&mut self, id: u64) -> Result<bool, Fault> {
        self.transition(id, Phase::Running, State::Held)
    }
    fn transition(&mut self, id: u64, from: Phase, to: State) -> Result<bool, Fault> {
        let job = self.records.get_mut(&id).ok_or(Fault::Unknown)?;
        if job.state.terminal() {
            return Ok(false);
        }
        if job.phase() != from {
            return Err(Fault::Transition);
        }
        job.state = to;
        Ok(true)
    }
    pub fn finish(&mut self, id: u64, result: Result<String, String>) -> Result<bool, Fault> {
        self.stamped(id, (result, None))
    }
    pub fn stamped(
        &mut self,
        id: u64,
        completion: (Result<String, String>, Option<std::time::Instant>),
    ) -> Result<bool, Fault> {
        let (result, produced) = completion;
        let job = self.records.get_mut(&id).ok_or(Fault::Unknown)?;
        if job.state.terminal() {
            return Ok(false);
        }
        if job.phase() == Phase::Queued {
            return Err(Fault::Transition);
        }
        job.state = State::Complete { result };
        job.produced = produced;
        self.trim();
        Ok(true)
    }
    pub fn close(&mut self, reason: &str) {
        self.closed = true;
        for job in self
            .records
            .values_mut()
            .filter(|job| !job.state.terminal())
        {
            job.state = State::Cancelled {
                reason: reason.into(),
            };
        }
        self.trim();
    }
    pub fn get(&self, id: u64) -> Option<&Job<K>> {
        self.records.get(&id)
    }
    pub fn snapshot(&self) -> Vec<Job<K>>
    where
        K: Clone,
    {
        self.records.values().cloned().collect()
    }
    fn trim(&mut self) {
        while self.records.len() > self.limit {
            let Some(id) = self
                .records
                .values()
                .find(|job| job.state.terminal())
                .map(|job| job.id)
            else {
                break;
            };
            self.records.remove(&id);
        }
    }
}
