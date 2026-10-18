use super::connection::Connection;
use super::event::monotonic;
use super::{Error, error};
use graphics::shared::{Pacing, Pulse};
use std::cell::Cell;
use std::time::{Duration, Instant};
use x11rb::connection::Connection as _;
use x11rb::protocol::present::{self, CompleteKind, CompleteNotifyEvent, EventMask};

pub(super) struct Timing {
    present: Option<Source>,
    pending: Cell<bool>,
    latest: Cell<Option<(u64, Instant)>>,
    vblank: Cell<Option<(u64, u64)>>,
    period: Cell<Option<Duration>>,
}

struct Source {
    eid: u32,
    window: u32,
}

impl Timing {
    pub fn new(connection: &Connection) -> Result<Self, Error> {
        let present = match present::query_version(&connection.conn, 1, 2) {
            Ok(cookie) => cookie.reply().ok().map(|_| ()),
            Err(_) => None,
        };
        let present = match present {
            Some(()) => {
                let eid = connection.conn.generate_id().map_err(error)?;
                present::select_input(
                    &connection.conn,
                    eid,
                    connection.root,
                    EventMask::COMPLETE_NOTIFY,
                )
                .map_err(error)?
                .check()
                .map_err(error)?;
                Some(Source {
                    eid,
                    window: connection.root,
                })
            }
            None => None,
        };
        Ok(Self {
            present,
            pending: Cell::new(false),
            latest: Cell::new(None),
            vblank: Cell::new(None),
            period: Cell::new(None),
        })
    }

    #[cfg(test)]
    pub fn timer() -> Self {
        Self {
            present: None,
            pending: Cell::new(false),
            latest: Cell::new(None),
            vblank: Cell::new(None),
            period: Cell::new(None),
        }
    }

    pub fn native(&self) -> bool {
        self.present.is_some()
    }

    pub fn request(&self, connection: &Connection) -> Result<(), Error> {
        let Some(source) = &self.present else {
            return Ok(());
        };
        if self.pending.get() {
            return Ok(());
        }
        let target = self.latest.get().map_or(0, |(msc, _)| msc + 1);
        present::notify_msc(&connection.conn, source.window, 0, target, 0, 0).map_err(error)?;
        self.pending.set(true);
        Ok(())
    }

    pub fn observe(&self, event: &CompleteNotifyEvent) -> bool {
        let Some(source) = &self.present else {
            return false;
        };
        if event.event != source.eid || event.kind != CompleteKind::NOTIFY_MSC {
            return false;
        }
        self.pending.set(false);
        self.settle(event.msc, event.ust, monotonic(), Instant::now());
        true
    }

    fn settle(&self, msc: u64, ust: u64, now: u64, observed: Instant) {
        let period = self
            .vblank
            .get()
            .and_then(|last| interval(last, msc, ust))
            .or(self.period.get());
        self.period.set(period);
        if ust != 0 {
            self.vblank.set(Some((msc, ust)));
        }
        let target = vblank(ust, now, observed)
            .zip(period)
            .map_or(observed, |(at, period)| at + period);
        let floor = self.latest.get().map_or(target, |(_, at)| at.max(target));
        self.latest.set(Some((msc, floor)));
    }

    pub fn pulse(&self, seen: &mut u64, pacing: Pacing) -> Option<Pulse> {
        if self.present.is_none() {
            return due(pacing).then(Pulse::now);
        }
        let (msc, at) = self.latest.get()?;
        if msc == *seen {
            return None;
        }
        *seen = msc;
        due(pacing).then(|| Pulse::new(at))
    }
}

fn interval(last: (u64, u64), msc: u64, ust: u64) -> Option<Duration> {
    let frames = msc.checked_sub(last.0).filter(|frames| *frames > 0)?;
    let micros = ust.checked_sub(last.1)? / frames;
    (1_000..=1_000_000)
        .contains(&micros)
        .then(|| Duration::from_micros(micros))
}

fn vblank(ust: u64, now: u64, observed: Instant) -> Option<Instant> {
    let age = now
        .checked_sub(ust)
        .filter(|age| ust != 0 && *age <= 1_000_000)?;
    observed.checked_sub(Duration::from_micros(age))
}

pub(super) fn due(pacing: Pacing) -> bool {
    match pacing {
        Pacing::Continuous => true,
        Pacing::At(deadline) => Instant::now() >= deadline,
        Pacing::Idle => false,
    }
}

#[path = "../../tests/linux/timing.rs"]
mod checks;
