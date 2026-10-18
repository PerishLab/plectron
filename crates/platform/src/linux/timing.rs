use super::connection::Connection;
use super::{Error, error};
use graphics::shared::{Pacing, Pulse};
use std::cell::Cell;
use std::time::Instant;
use x11rb::connection::Connection as _;
use x11rb::protocol::present::{self, CompleteKind, CompleteNotifyEvent, EventMask};

pub(super) struct Timing {
    present: Option<Source>,
    pending: Cell<bool>,
    latest: Cell<Option<(u64, Instant)>>,
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
        })
    }

    #[cfg(test)]
    pub fn timer() -> Self {
        Self {
            present: None,
            pending: Cell::new(false),
            latest: Cell::new(None),
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
        self.latest.set(Some((event.msc, Instant::now())));
        true
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

pub(super) fn due(pacing: Pacing) -> bool {
    match pacing {
        Pacing::Continuous => true,
        Pacing::At(deadline) => Instant::now() >= deadline,
        Pacing::Idle => false,
    }
}

#[path = "../../tests/linux/timing.rs"]
mod checks;
