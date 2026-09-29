mod binary;
mod duplex;
use crate::Fault;
pub use binary::Client;
pub use duplex::Duplex;
use serde::{Deserialize, Serialize};
use std::io::Read;

pub const CHUNK: usize = 65_536;
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Command {
    Read { limit: usize },
    Close,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Chunk {
    pub bytes: Vec<u8>,
    pub eof: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Open,
    Ended,
    Closed,
    Failed,
}
pub struct Transfer<R> {
    source: Option<R>,
    remaining: u64,
    state: State,
    observation: crate::observe::Lease,
    monitor: crate::observe::Monitor,
}
impl<R: Read> Transfer<R> {
    pub fn new(source: R, length: u64) -> Self {
        Self::tracked(source, length, &crate::observe::Monitor::default())
    }
    pub fn tracked(source: R, length: u64, monitor: &crate::observe::Monitor) -> Self {
        let monitor = monitor.clone();
        let observation = monitor.lease();
        Self {
            observation,
            monitor,
            source: Some(source),
            remaining: length,
            state: State::Open,
        }
    }
    pub fn monitor(&self) -> crate::observe::Monitor {
        self.monitor.clone()
    }
    pub fn state(&self) -> State {
        self.state
    }
    pub fn read(&mut self, limit: usize) -> Result<Chunk, Fault> {
        if limit == 0 || limit > CHUNK {
            return Err(Fault("invalid stream chunk budget".into()));
        }
        if matches!(self.state, State::Closed | State::Failed) {
            return Err(Fault("transfer is unavailable".into()));
        }
        let length = self.remaining.min(limit as u64) as usize;
        let mut bytes = vec![0; length];
        let source = self
            .source
            .as_mut()
            .ok_or_else(|| Fault("transfer is closed".into()))?;
        if let Err(error) = source.read_exact(&mut bytes) {
            self.observation
                .fail("source ended before advertised length");
            self.state = State::Failed;
            self.source = None;
            return Err(error.into());
        }
        self.observation.bytes(length);
        self.remaining -= length as u64;
        let eof = self.remaining == 0;
        if eof {
            self.state = State::Ended;
        }
        Ok(Chunk { bytes, eof })
    }
    pub fn close(&mut self) {
        if self.state == State::Ended {
            self.observation.finish();
        } else {
            self.observation.cancel("receiver closed before EOF");
        }
        self.source = None;
        self.state = State::Closed;
    }
}
