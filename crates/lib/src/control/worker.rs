use super::protocol::{Action, Report, Task};
use super::proxy::{Envelope, Proxy};
use super::{Kind, Snapshot, execution};
use crate::contract::{Fault, Signal};
use crate::platform::Host;
use shared::budget::Budget;
use shared::gate::Gate;
use shared::jobs::Jobs;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};

static WORKERS: Budget = Budget::new(8);
const DRAIN: usize = 16;

pub struct Worker {
    activity: jsengine::Activity,
    performance: jsengine::jit::Report,
    waiting: Arc<AtomicBool>,
    sender: Option<SyncSender<Task>>,
    receiver: Option<Receiver<observe::Envelope<Report>>>,
    port: Option<Receiver<Envelope>>,
    gate: Gate,
    queue: VecDeque<Task>,
    jobs: Jobs<Kind>,
    startup: Option<Result<(), String>>,
    sequence: u64,
    current: Option<u64>,
    fault: Option<String>,
    failure: Option<String>,
}
impl Worker {
    pub fn spawn(source: &str, notify: Option<shared::Notify>) -> Result<Self, Fault> {
        let permit = WORKERS
            .acquire()
            .ok_or_else(|| Fault("control worker budget exhausted".into()))?;
        let (proxy, port) = Proxy::channel();
        let proxy = match &notify {
            Some(notify) => proxy.notified(notify.clone()),
            None => proxy,
        };
        let waiting = proxy.waiting();
        let (sender, inbox) = mpsc::sync_channel(1);
        let (outbox, receiver) = mpsc::sync_channel(2);
        std::thread::Builder::new()
            .name("plectron-control".into())
            .spawn(move || {
                let _permit = permit;
                execution::run(
                    proxy,
                    inbox,
                    super::publication::Publisher::new(outbox, notify),
                );
            })
            .map_err(|error| Fault(error.to_string()))?;
        let mut worker = Self {
            activity: Default::default(),
            performance: Default::default(),
            waiting,
            sender: Some(sender),
            receiver: Some(receiver),
            port: Some(port),
            gate: Gate::default(),
            queue: VecDeque::new(),
            jobs: Jobs::new(64),
            startup: None,
            sequence: 0,
            current: None,
            fault: None,
            failure: None,
        };
        worker.submit(Action::Startup(source.into()))?;
        Ok(worker)
    }
    pub fn evaluate(&mut self, source: &str) -> Result<u64, Fault> {
        self.submit(Action::Evaluate(source.into()))
    }
    pub fn dispatch(&mut self, signal: Signal) -> Result<u64, Fault> {
        self.submit(Action::Dispatch(signal))
    }
    fn submit(&mut self, action: Action) -> Result<u64, Fault> {
        if let Action::Startup(source) | Action::Evaluate(source) = &action
            && source.len() > 65_536
        {
            return Err(Fault("control source budget exhausted".into()));
        }
        if self.sender.is_none() || self.queue.len() >= 8 {
            return Err(Fault("control mailbox unavailable".into()));
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Fault("control identifiers exhausted".into()))?;
        let id = self.sequence;
        let kind = action.kind();
        self.jobs
            .admit(id, kind)
            .map_err(|error| Fault(error.to_string()))?;
        self.queue.push_back(Task {
            id,
            action,
            context: observe::Transfer::capture(),
        });
        Ok(id)
    }
    pub fn hold(&mut self) -> Result<u64, Fault> {
        let ticket = self.gate.hold().map_err(|error| Fault(error.to_string()))?;
        let result = self.submit(Action::Hold(ticket));
        if result.is_err() {
            let _ = self.gate.release();
        }
        result
    }
    pub fn unhold(&mut self) -> Result<(), Fault> {
        self.gate
            .release()
            .map_err(|error| Fault(error.to_string()))
    }
    pub fn poll(&mut self, host: &mut impl Host) {
        if let Some(port) = &self.port {
            for request in port.try_iter().take(16) {
                request.apply(host);
            }
        }
        for _ in 0..DRAIN {
            if !self.receive() {
                break;
            }
        }
        self.advance();
    }
    fn receive(&mut self) -> bool {
        let Some(receiver) = &self.receiver else {
            return false;
        };
        match receiver.try_recv() {
            Ok(report) => {
                let (report, mut scope) = report.receive("control.accept");
                let result = self.accept(report);
                scope.finish(result.is_ok());
                if let Err(error) = result {
                    self.fail(&error.to_string());
                }
                true
            }
            Err(TryRecvError::Empty) => false,
            Err(TryRecvError::Disconnected) => {
                self.fail("control worker disconnected");
                false
            }
        }
    }
    fn advance(&mut self) {
        if self.current.is_some() {
            return;
        }
        let Some(task) = self.queue.pop_front() else {
            return;
        };
        let Some(sender) = &self.sender else {
            return;
        };
        let id = task.id;
        if sender.try_send(task).is_ok() {
            self.current = Some(id);
        } else {
            self.fail("control worker disconnected");
        }
    }
    fn accept(&mut self, report: Report) -> Result<(), shared::jobs::Fault> {
        match report {
            Report::Started(id) => {
                self.jobs.start(id)?;
            }
            Report::Held(id) => {
                self.jobs.hold(id)?;
            }
            Report::Finished(id, result, activity, performance, produced) => {
                self.activity = activity;
                self.performance = performance;
                let kind = self.jobs.get(id).ok_or(shared::jobs::Fault::Unknown)?.kind;
                let error = result.as_ref().err().cloned();
                if !self.jobs.stamped(id, (result, Some(produced)))? {
                    return Ok(());
                }
                match kind {
                    Kind::Dispatch if error.is_some() => self.failure = error,
                    Kind::Startup => self.startup = Some(error.map_or(Ok(()), Err)),
                    _ => (),
                }
                self.current = None;
            }
        }
        Ok(())
    }
    fn fail(&mut self, reason: &str) {
        self.fault = Some(reason.into());
        self.failure = self.fault.clone();
        self.close();
    }
    pub fn job(&self, id: u64) -> Option<&super::protocol::Job> {
        self.jobs.get(id)
    }
    pub fn inspect(&self) -> Snapshot {
        Snapshot {
            activity: self.activity,
            performance: self.performance.clone(),
            waiting: self.port.is_some() && self.waiting.load(Ordering::Acquire),
            jobs: self.jobs.snapshot(),
            fault: self.fault.clone(),
        }
    }
    pub fn busy(&self) -> bool {
        self.current.is_some() || !self.queue.is_empty()
    }
    pub fn available(&self) -> bool {
        self.sender.is_some() && !self.busy()
    }
    pub fn failure(&mut self) -> Option<String> {
        self.failure.take()
    }
    pub fn startup(&self) -> Result<(), Fault> {
        match &self.startup {
            Some(Err(error)) => Err(Fault(error.clone())),
            _ => Ok(()),
        }
    }
    pub fn close(&mut self) {
        self.port = None;
        self.sender = None;
        self.receiver = None;
        self.gate.close();
        self.queue.clear();
        self.current = None;
        let reason = self
            .fault
            .as_deref()
            .unwrap_or("control closed before completion");
        self.jobs.close(reason);
        if self.startup.is_none() {
            self.startup = Some(Err(reason.into()));
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.close();
    }
}
