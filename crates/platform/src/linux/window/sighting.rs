use super::super::timing::presented;
use std::collections::VecDeque;
use std::time::Instant;
use x11rb::protocol::present::{CompleteKind, CompleteMode, CompleteNotifyEvent};

const AWAITED: usize = 64;

#[derive(Default)]
pub(super) struct Sightings {
    last: u32,
    awaited: VecDeque<(u32, u64)>,
    seen: Vec<(u64, Instant)>,
}

impl Sightings {
    pub fn serial(&mut self) -> u32 {
        self.last = self.last.wrapping_add(1);
        self.last
    }

    pub fn watch(&mut self, serial: u32, frame: u64) {
        if self.awaited.len() == AWAITED {
            self.awaited.pop_front();
        }
        self.awaited.push_back((serial, frame));
    }

    pub fn settle(
        &mut self,
        event: &CompleteNotifyEvent,
        mapped: bool,
        now: u64,
        observed: Instant,
    ) {
        if event.kind != CompleteKind::PIXMAP {
            return;
        }
        let Some(index) = self
            .awaited
            .iter()
            .position(|(serial, _)| *serial == event.serial)
        else {
            return;
        };
        let Some((_, frame)) = self.awaited.remove(index) else {
            return;
        };
        if event.mode == CompleteMode::SKIP || !mapped {
            return;
        }
        if let Some(at) = presented(event.ust, now, observed) {
            self.seen.push((frame, at));
        }
    }

    pub fn take(&mut self) -> Vec<(u64, Instant)> {
        std::mem::take(&mut self.seen)
    }

    #[cfg(test)]
    pub fn awaiting(&self) -> usize {
        self.awaited.len()
    }
}

#[path = "../../../tests/linux/sighting.rs"]
mod checks;
#[path = "../../../tests/linux/visible.rs"]
mod visible;
