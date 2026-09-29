use crate::{Context, Guard, Kind, Options, Report};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

static IDENTITIES: AtomicU64 = AtomicU64::new(1);
pub(crate) fn identity() -> u64 {
    IDENTITIES.fetch_add(1, Ordering::Relaxed)
}
#[derive(Clone)]
pub struct Session(pub(crate) Arc<Inner>);
pub(crate) struct Inner {
    pub id: u64,
    pub options: Options,
    pub start: Instant,
    pub state: Mutex<State>,
}
pub(crate) struct State {
    pub stopped: bool,
    pub admitted: usize,
    pub open: usize,
    pub dropped: usize,
    pub records: Vec<crate::Record>,
}
impl Session {
    pub fn new(options: Options) -> Self {
        Self(Arc::new(Inner {
            id: identity(),
            options,
            start: Instant::now(),
            state: Mutex::new(State {
                stopped: false,
                admitted: 0,
                open: 0,
                dropped: 0,
                records: Vec::new(),
            }),
        }))
    }
    pub fn scope(&self, name: impl Into<String>) -> Guard {
        Context {
            session: self.clone(),
            parent: None,
        }
        .open(name.into(), Kind::Scope)
    }
    pub fn stop(&self) {
        self.0.state.lock().expect("observation state").stopped = true;
    }
    pub fn status(&self) -> crate::Status {
        let state = self.0.state.lock().expect("observation state");
        crate::Status {
            session: self.0.id,
            options: self.0.options,
            stopped: state.stopped,
            open: state.open,
            dropped: state.dropped,
            recorded: state.records.len(),
        }
    }
    pub fn inspect(&self) -> Report {
        let state = self.0.state.lock().expect("observation state");
        Report {
            version: 1,
            unit: "nanoseconds".into(),
            clock: "session-relative monotonic; cpu is separately sampled thread time".into(),
            process: std::process::id(),
            session: self.0.id,
            options: self.0.options,
            stopped: state.stopped,
            open: state.open,
            dropped: state.dropped,
            suppression: "rejected scope entries; their descendants are suppressed and uncounted"
                .into(),
            native: cfg!(target_os = "macos"),
            records: state.records.clone(),
        }
    }
}
impl Inner {
    pub fn elapsed(&self) -> u64 {
        self.start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
    }
}
