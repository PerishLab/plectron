use serde::Serialize;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Snapshot {
    pub bytes: u64,
    pub active: u64,
    pub completed: u64,
    pub cancelled: u64,
    pub failed: u64,
    pub waits: u64,
    pub waiting: u64,
    pub first: Option<u64>,
    pub reason: Option<String>,
}
#[derive(Clone, Default)]
pub struct Monitor(Arc<Mutex<Snapshot>>);
pub struct Lease {
    monitor: Monitor,
    started: Instant,
    first: bool,
    ended: bool,
}
impl Monitor {
    pub fn snapshot(&self) -> Snapshot {
        self.0.lock().expect("monitor lock").clone()
    }
    pub fn lease(&self) -> Lease {
        self.0.lock().expect("monitor lock").active += 1;
        Lease {
            monitor: self.clone(),
            started: Instant::now(),
            first: true,
            ended: false,
        }
    }
}
impl Lease {
    pub fn bytes(&mut self, bytes: usize) {
        let mut snapshot = self.monitor.0.lock().expect("monitor lock");
        snapshot.bytes += bytes as u64;
        if self.first && bytes > 0 {
            snapshot.first = Some(self.started.elapsed().as_nanos().min(u64::MAX as u128) as u64);
            self.first = false;
        }
    }
    pub fn wait(&self, started: Instant) {
        let mut snapshot = self.monitor.0.lock().expect("monitor lock");
        snapshot.waits += 1;
        snapshot.waiting += started.elapsed().as_nanos().min(u64::MAX as u128) as u64;
    }
    pub fn finish(&mut self) {
        self.end(None, false);
    }
    pub fn fail(&mut self, reason: &str) {
        self.end(Some(reason), true);
    }
    pub fn cancel(&mut self, reason: &str) {
        self.end(Some(reason), false);
    }
    fn end(&mut self, reason: Option<&str>, failed: bool) {
        if self.ended {
            return;
        }
        self.ended = true;
        let mut snapshot = self.monitor.0.lock().expect("monitor lock");
        snapshot.active -= 1;
        match reason {
            None => snapshot.completed += 1,
            Some(reason) => {
                snapshot.reason = Some(reason.into());
                if failed {
                    snapshot.failed += 1;
                } else {
                    snapshot.cancelled += 1;
                }
            }
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        self.cancel("owner dropped before completion");
    }
}
