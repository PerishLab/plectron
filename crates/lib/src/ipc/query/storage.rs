use super::{Command, Record, Reply, State, Status, Target};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::thread::Thread;
use std::time::Instant;

pub const HISTORY: usize = 64;

pub struct Production {
    pub produced: Option<Instant>,
    pub visible: Option<Instant>,
    pub drops: u64,
}

impl From<Option<Instant>> for Production {
    fn from(produced: Option<Instant>) -> Self {
        Self {
            produced,
            visible: None,
            drops: 0,
        }
    }
}

#[derive(Clone)]
pub struct Store(Arc<Mutex<Storage>>);
#[derive(Clone)]
pub struct Reader(Store);
struct Storage {
    epoch: Instant,
    sequence: u64,
    version: u64,
    records: BTreeMap<u64, Arc<Record>>,
    latest: BTreeMap<u64, Arc<Record>>,
    limit: usize,
    closed: bool,
    notify: Option<Thread>,
}
impl Default for Store {
    fn default() -> Self {
        Self::new(HISTORY)
    }
}
impl Store {
    pub fn new(limit: usize) -> Self {
        Self(Arc::new(Mutex::new(Storage {
            epoch: Instant::now(),
            sequence: 0,
            version: 0,
            records: BTreeMap::new(),
            latest: BTreeMap::new(),
            limit,
            closed: false,
            notify: None,
        })))
    }
    pub fn reader(&self) -> Reader {
        Reader(self.clone())
    }
    fn notify(&self, thread: Thread) -> Result<(), String> {
        let mut state = self.0.lock().expect("query storage");
        if state.notify.is_some() {
            return Err("query service already attached".into());
        }
        state.notify = Some(thread);
        Ok(())
    }
    pub fn admit(&self, target: Target) -> Result<u64, String> {
        self.admitted(target, Instant::now())
    }
    pub fn admitted(&self, target: Target, admitted: Instant) -> Result<u64, String> {
        let mut state = self.0.lock().expect("query storage");
        if state.closed {
            return Err("query publication closed".into());
        }
        state.sequence = state.sequence.checked_add(1).ok_or("record overflow")?;
        let id = state.sequence;
        let now = state.time(Instant::now());
        let admitted = state.time(admitted);
        state.publish(Record {
            id,
            version: 0,
            target,
            state: State::Pending,
            admitted,
            produced: None,
            visible: None,
            seen: None,
            drops: 0,
            detected: now,
        });
        Ok(id)
    }
    pub fn finish(&self, id: u64, result: State, production: impl Into<Production>) {
        let production = production.into();
        let mut state = self.0.lock().expect("query storage");
        let Some(record) = state.records.get(&id).cloned().or_else(|| {
            state
                .latest
                .values()
                .find(|record| record.id == id)
                .cloned()
        }) else {
            return;
        };
        if record.state != State::Pending || result == State::Pending {
            return;
        }
        let mut record = (*record).clone();
        record.state = result;
        record.produced = production.produced.map(|time| state.time(time));
        record.visible = production.visible.map(|time| state.time(time));
        record.seen = production.visible.map(|_| state.time(Instant::now()));
        record.drops = production.drops;
        record.detected = state.time(Instant::now());
        state.publish(record);
    }
    pub fn visible(&self, id: u64, visible: Instant, drops: u64) {
        let mut state = self.0.lock().expect("query storage");
        let Some(record) = state.records.get(&id).cloned().or_else(|| {
            state
                .latest
                .values()
                .find(|record| record.id == id)
                .cloned()
        }) else {
            return;
        };
        if !matches!(record.target, Target::Frame { .. }) || record.visible.is_some() {
            return;
        }
        let mut record = (*record).clone();
        record.visible = Some(state.time(visible));
        record.seen = Some(state.time(Instant::now()));
        record.drops = drops;
        state.publish(record);
    }
    pub fn frame(&self, unit: u64, frame: u64) -> Option<u64> {
        let state = self.0.lock().expect("query storage");
        state.records.values().find_map(|record| {
            matches!(record.target, Target::Frame { unit: u, frame: f, .. } if (u,f) == (unit,frame))
                .then_some(record.id)
        }).or_else(|| {
            state.latest.get(&unit).and_then(|record| {
                matches!(record.target, Target::Frame { frame: f, .. } if f == frame)
                    .then_some(record.id)
            })
        })
    }
    pub fn cancel(&self, unit: Option<u64>) {
        let mut state = self.0.lock().expect("query storage");
        let pending: Vec<_> = state
            .records
            .values()
            .filter(|record| {
                record.state == State::Pending && (unit.is_none() || record.target.unit() == unit)
            })
            .cloned()
            .collect();
        for record in pending {
            let mut record = (*record).clone();
            record.state = State::Cancelled {
                reason: "owner closed".into(),
            };
            record.detected = state.time(Instant::now());
            state.publish(record);
        }
        if let Some(unit) = unit {
            state.latest.remove(&unit);
        } else {
            state.closed = true;
            state.latest.clear();
        }
        state.version += 1;
        state.wake();
    }
    pub fn read(&self, command: &Command) -> Reply {
        let (record, fallback, version, replied, closed) = {
            let state = self.0.lock().expect("query storage");
            let (record, fallback) = state.select(command);
            (
                record,
                fallback,
                state.version,
                state.time(Instant::now()),
                state.closed,
            )
        };
        Reply {
            status: record.map_or(fallback, |record| Status::Record {
                record: (*record).clone(),
            }),
            version,
            replied,
            timedout: false,
            closed,
        }
    }
}
impl Storage {
    fn select(&self, command: &Command) -> (Option<Arc<Record>>, Status) {
        match command {
            Command::Latest { unit } => (
                self.latest.get(unit).cloned(),
                Status::Absent { unit: *unit },
            ),
            Command::Read { record } | Command::Wait { record, .. } => {
                let fallback = if *record > 0 && *record <= self.sequence {
                    Status::Evicted { id: *record }
                } else {
                    Status::Unknown { id: *record }
                };
                (self.records.get(record).cloned(), fallback)
            }
        }
    }

    fn time(&self, time: Instant) -> u64 {
        time.saturating_duration_since(self.epoch)
            .as_nanos()
            .try_into()
            .unwrap_or(u64::MAX)
    }
    fn publish(&mut self, mut record: Record) {
        self.version += 1;
        record.version = self.version;
        let record = Arc::new(record);
        if let Target::Frame { unit, .. } = record.target {
            let current = self.latest.get(&unit).map_or(0, |record| record.id);
            if record.id >= current {
                self.latest.insert(unit, record.clone());
            }
        }
        self.records.insert(record.id, record);
        while self
            .records
            .values()
            .filter(|record| record.state != State::Pending)
            .count()
            > self.limit
        {
            let id = self
                .records
                .values()
                .find(|record| record.state != State::Pending)
                .expect("terminal record")
                .id;
            self.records.remove(&id);
        }
        self.wake();
    }
    fn wake(&self) {
        if let Some(thread) = &self.notify {
            thread.unpark();
        }
    }
}

impl Reader {
    pub fn read(&self, command: &Command) -> Reply {
        self.0.read(command)
    }
    pub(super) fn attach(&self, thread: Thread) -> Result<(), String> {
        self.0.notify(thread)
    }
    pub(super) fn detach(&self) {
        self.0.0.lock().expect("query storage").notify = None;
    }
}
