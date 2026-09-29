use super::{Execution, Worker};
use crate::{Fault, Snapshot};

impl Worker {
    pub fn pacing(&self) -> Result<graphics::shared::Pacing, Fault> {
        self.animation.pacing()
    }

    pub fn pulse(&self, pulse: graphics::shared::Pulse) -> Result<bool, Fault> {
        self.animation.pulse(pulse)
    }

    pub(super) fn animate(&mut self) -> Result<(), Fault> {
        if let Some(fault) = self.animation.fault()? {
            self.diagnostics.push(fault);
            self.display.reset()?;
        }
        if let Some(effect) = self.animation.take()? {
            observe::call("animation.install", || self.display.effect(effect))?;
        }
        Ok(())
    }

    pub fn inspect(&self) -> Snapshot {
        let mut snapshot = self.display.inspect();
        if let Ok(statistics) = self.animation.statistics() {
            snapshot.computation.animations = statistics.animations;
            snapshot.computation.effects = statistics.effects;
            snapshot.computation.coalesced = statistics.coalesced;
            snapshot.computation.discarded = statistics.discarded;
            snapshot.computation.submitted = statistics.submitted;
            snapshot.computation.mailbox = statistics.mailbox;
            snapshot.computation.sampling = statistics.sampling;
            snapshot.computation.pulses = statistics.pulses;
            snapshot.computation.superseded = statistics.superseded;
        }
        snapshot.execution = Some(Execution {
            jobs: self.jobs.snapshot(),
            fault: self.fault.clone(),
        });
        snapshot
    }
}
