use super::{CAPACITY, Command};
use crate::Fault;
use domengine::style::animation::Schedule;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::Instant;

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Statistics {
    pub animations: u64,
    pub effects: u64,
    pub coalesced: u64,
    pub discarded: u64,
    pub submitted: u64,
    pub mailbox: u64,
    pub sampling: u64,
    pub pulses: u64,
    pub superseded: u64,
}

#[derive(Clone, Copy)]
pub(super) struct Sample {
    pub generation: u64,
    pub now: u64,
}

#[derive(Default)]
pub(super) struct Shared {
    pub effect: Option<graphics::shared::Effects>,
    pub fault: Option<String>,
    pub statistics: Statistics,
    pub schedule: Option<Schedule>,
    pub generation: u64,
    pub sample: Option<Sample>,
    pub marker: bool,
}

#[derive(Clone)]
pub(super) struct Channel {
    pub sender: mpsc::SyncSender<Command>,
    pub pending: Arc<AtomicUsize>,
    pub shared: Arc<Mutex<Shared>>,
    pub clock: Instant,
}

pub(crate) struct Reader {
    pub(super) channel: Channel,
}

impl Channel {
    pub fn send(&self, command: Command) -> Result<(), Fault> {
        let pending = self.pending.fetch_add(1, Ordering::AcqRel) + 1;
        if let Ok(mut shared) = self.shared.lock() {
            shared.statistics.mailbox = shared.statistics.mailbox.max(pending.min(CAPACITY) as u64);
        }
        if self.sender.send(command).is_err() {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            return Err(Fault("animation executor disconnected".into()));
        }
        Ok(())
    }

    fn pulse(&self, pulse: graphics::shared::Pulse) -> Result<bool, Fault> {
        let now = pulse
            .target()
            .checked_duration_since(self.clock)
            .unwrap_or_default()
            .as_micros()
            .min(u128::from(u64::MAX)) as u64;
        let mut shared = self.shared.lock().map_err(crate::error)?;
        let due = match shared.schedule {
            Some(Schedule::Continuous) => true,
            Some(Schedule::At(at)) => now >= at,
            None => false,
        };
        if !due {
            return Ok(false);
        }
        let sample = Sample {
            generation: shared.generation,
            now,
        };
        shared.statistics.pulses = shared.statistics.pulses.saturating_add(1);
        if shared.sample.replace(sample).is_some() {
            shared.statistics.superseded = shared.statistics.superseded.saturating_add(1);
        }
        if shared.marker {
            return Ok(true);
        }
        shared.marker = true;
        drop(shared);
        let pending = self.pending.fetch_add(1, Ordering::AcqRel) + 1;
        match self.sender.try_send(Command::Pulse) {
            Ok(()) => {
                if let Ok(mut shared) = self.shared.lock() {
                    shared.statistics.mailbox =
                        shared.statistics.mailbox.max(pending.min(CAPACITY) as u64);
                }
                Ok(true)
            }
            Err(mpsc::TrySendError::Full(_)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                Ok(true)
            }
            Err(mpsc::TrySendError::Disconnected(_)) => {
                self.pending.fetch_sub(1, Ordering::AcqRel);
                if let Ok(mut shared) = self.shared.lock() {
                    shared.marker = false;
                    shared.sample = None;
                }
                Err(Fault("animation executor disconnected".into()))
            }
        }
    }
}

impl Reader {
    pub fn pacing(&self) -> Result<graphics::shared::Pacing, Fault> {
        let shared = self.channel.shared.lock().map_err(crate::error)?;
        Ok(match shared.schedule {
            Some(Schedule::Continuous) => graphics::shared::Pacing::Continuous,
            Some(Schedule::At(at)) => {
                let deadline = self
                    .channel
                    .clock
                    .checked_add(std::time::Duration::from_micros(at));
                deadline.map_or(graphics::shared::Pacing::Idle, graphics::shared::Pacing::At)
            }
            None => graphics::shared::Pacing::Idle,
        })
    }

    pub fn pulse(&self, pulse: graphics::shared::Pulse) -> Result<bool, Fault> {
        self.channel.pulse(pulse)
    }

    pub fn take(&self) -> Result<Option<graphics::shared::Effects>, Fault> {
        let mut shared = self.channel.shared.lock().map_err(crate::error)?;
        let effect = shared.effect.take();
        if effect.is_some() {
            shared.statistics.submitted = shared.statistics.submitted.saturating_add(1);
        }
        Ok(effect)
    }

    pub fn fault(&self) -> Result<Option<String>, Fault> {
        let mut shared = self.channel.shared.lock().map_err(crate::error)?;
        let fault = shared.fault.take();
        if fault.is_some() {
            shared.effect = None;
        }
        Ok(fault)
    }

    pub fn statistics(&self) -> Result<Statistics, Fault> {
        Ok(self.channel.shared.lock().map_err(crate::error)?.statistics)
    }
}
