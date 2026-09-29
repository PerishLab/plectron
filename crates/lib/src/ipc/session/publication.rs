use super::Session;
use crate::contract::Fault;
use crate::ipc::{
    Command,
    query::{State, Target},
};
use crate::platform::Backend;
use crate::rendering::Renderer;

pub(super) struct Pending {
    record: u64,
    unit: Option<u64>,
    job: u64,
}
pub(super) fn target(command: &Command) -> Option<u64> {
    match command {
        Command::Evaluate { unit, .. }
        | Command::Collect { unit }
        | Command::Cache { unit, .. } => Some(*unit),
        Command::Hold { unit } => *unit,
        _ => None,
    }
}
impl<B: Backend, R: Renderer> Session<B, R> {
    pub(super) fn track(
        &mut self,
        unit: Option<u64>,
        job: u64,
        admitted: std::time::Instant,
    ) -> Result<u64, Fault> {
        let record = self
            .runtime
            .host()
            .query()
            .admitted(Target::Task { unit, job }, admitted)
            .map_err(Fault)?;
        self.records.push(Pending { record, unit, job });
        Ok(record)
    }
    pub(super) fn publish(&mut self) {
        let store = self.runtime.host().query();
        let records = std::mem::take(&mut self.records);
        for pending in records {
            let job = match pending.unit {
                Some(unit) => self
                    .runtime
                    .host()
                    .job(unit, pending.job)
                    .map(|job| (&job.state, job.produced)),
                None => self
                    .runtime
                    .job(pending.job)
                    .map(|job| (&job.state, job.produced)),
            };
            let Some((state, produced)) = job else {
                store.finish(
                    pending.record,
                    State::Unavailable {
                        reason: "job no longer retained by owner".into(),
                    },
                    None,
                );
                continue;
            };
            let state = match state {
                shared::jobs::State::Complete { result } => State::Complete {
                    result: result.clone(),
                },
                shared::jobs::State::Cancelled { reason } => State::Cancelled {
                    reason: reason.clone(),
                },
                _ => {
                    self.records.push(pending);
                    continue;
                }
            };
            store.finish(pending.record, state, produced);
        }
    }
}
