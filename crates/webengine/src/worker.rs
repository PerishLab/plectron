mod animation;
mod demand;
mod display;
mod execution;
mod protocol;
mod publication;
use crate::resource::Provider;
use crate::{Display, Fault, State};
use domengine::Extent;
pub use protocol::{Action, Execution, Job, Kind, Phase};
use protocol::{Report, Task, Turn};
use shared::budget::Budget;
use shared::gate::Gate;
use shared::jobs::Jobs;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};

const DRAIN: usize = 16;
static WORKERS: Budget = Budget::new(8);

pub struct Worker {
    notify: Option<shared::Notify>,
    demand: Option<u64>,
    resizing: Option<u64>,
    pending: usize,
    requested: Option<(u64, graphics::shared::Visual)>,
    display: Display,
    animation: crate::page::AnimationReader,
    sender: Option<SyncSender<Task>>,
    receiver: Option<Receiver<observe::Envelope<Report>>>,
    gate: Gate,
    closed: Arc<AtomicBool>,
    jobs: Jobs<Kind>,
    fault: Option<String>,
    messages: Vec<String>,
    errors: Vec<String>,
    diagnostics: Vec<String>,
}
impl Worker {
    pub fn spawn(
        url: &str,
        extent: Extent,
        provider: impl Provider + Send + 'static,
    ) -> Result<Self, Fault> {
        Self::configured(url, extent, provider, config::Store::default())
    }
    pub fn configured(
        url: &str,
        extent: Extent,
        provider: impl Provider + Send + 'static,
        config: config::Store,
    ) -> Result<Self, Fault> {
        Self::notified(url, extent, provider, (config, None))
    }
    pub fn notified(
        url: &str,
        extent: Extent,
        provider: impl Provider + Send + 'static,
        options: (config::Store, Option<shared::Notify>),
    ) -> Result<Self, Fault> {
        let (config, notify) = options;
        let notification = notify.clone();
        let display = Display::new(url, extent)?;
        let clock = std::time::Instant::now();
        let (animation, animation_reader) = crate::page::spawn_animation(clock, notify.clone())?;
        let permit = WORKERS
            .acquire()
            .ok_or_else(|| Fault("content worker budget exhausted".into()))?;
        let (sender, inbox) = mpsc::sync_channel(8);
        let (outbox, receiver) = mpsc::sync_channel(8);
        let closed = Arc::new(AtomicBool::new(false));
        let cancellation = closed.clone();
        let url = url.to_string();
        let id = protocol::identifier()?;
        let mut jobs = Jobs::new(64);
        jobs.admit(id, Kind::Load).map_err(crate::error)?;
        let context = observe::current();
        std::thread::Builder::new()
            .name("plectron-content".into())
            .spawn(move || {
                let _permit = permit;
                execution::run(
                    execution::Source {
                        url,
                        extent,
                        provider,
                    },
                    execution::Channels {
                        id,
                        inbox,
                        outbox: publication::Publisher::new(outbox, notify),
                        context,
                    },
                    cancellation,
                    execution::Setup {
                        config,
                        clock,
                        animation,
                    },
                );
            })
            .map_err(crate::error)?;
        Ok(Self {
            notify: notification,
            demand: None,
            resizing: None,
            pending: 1,
            requested: None,
            display,
            animation: animation_reader,
            sender: Some(sender),
            receiver: Some(receiver),
            gate: Gate::default(),
            closed,
            jobs,
            fault: None,
            messages: Vec::new(),
            errors: Vec::new(),
            diagnostics: Vec::new(),
        })
    }
    pub fn submit(&mut self, action: Action) -> Result<u64, Fault> {
        action.validate()?;
        let resize = matches!(action, Action::Resize(_));
        let id = self.enqueue(Turn::Execute(action))?;
        if resize {
            self.resizing = Some(id);
        }
        Ok(id)
    }
    pub fn hold(&mut self) -> Result<u64, Fault> {
        let ticket = self.gate.hold().map_err(crate::error)?;
        let result = self.enqueue(Turn::Hold(ticket));
        if result.is_err() {
            let _ = self.gate.release();
        }
        result
    }
    fn enqueue(&mut self, turn: Turn) -> Result<u64, Fault> {
        self.display.check()?;
        if self.fault.is_some() {
            return Err(Fault("content worker failed".into()));
        }
        let kind = turn.kind();
        let id = protocol::identifier()?;
        self.sender
            .as_ref()
            .ok_or_else(|| Fault("content worker closed".into()))?
            .try_send(Task {
                id,
                turn,
                context: observe::Transfer::capture(),
            })
            .map_err(|_| Fault("content mailbox unavailable".into()))?;
        self.jobs.admit(id, kind).map_err(crate::error)?;
        self.pending += 1;
        Ok(id)
    }
    pub fn unhold(&mut self) -> Result<(), Fault> {
        self.display.check()?;
        if self.sender.is_none() {
            return Err(Fault("content worker closed".into()));
        }
        self.gate.release().map_err(crate::error)
    }
    pub fn poll(&mut self) -> Result<(), Fault> {
        self.animate()?;
        self.flush()?;
        if !self.messages.is_empty() || !self.errors.is_empty() {
            return Ok(());
        }
        let mut drained = 0;
        for _ in 0..DRAIN {
            let Some(receiver) = &self.receiver else {
                break;
            };
            match receiver.try_recv() {
                Ok(report) => {
                    let (report, mut scope) = report.receive("content.accept");
                    let result = self.accept(report);
                    scope.finish(result.is_ok());
                    result?;
                    drained += 1;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.fail("content worker disconnected");
                    break;
                }
            }
            if !self.messages.is_empty() || !self.errors.is_empty() {
                break;
            }
        }
        if (drained == DRAIN || !self.messages.is_empty() || !self.errors.is_empty())
            && let Some(notify) = &self.notify
        {
            notify.notify();
        }
        self.request()
    }
    fn accept(&mut self, report: Report) -> Result<(), Fault> {
        match report {
            Report::Update {
                publication,
                messages,
                result,
            } => {
                if let Err(fault) = result {
                    self.errors.push(fault);
                }
                observe::call("content.install", || self.display.install(*publication))?;
                self.messages.extend(messages);
            }
            Report::Held(id) => {
                self.jobs.hold(id).map_err(crate::error)?;
            }
            Report::Started(id) => {
                self.jobs.start(id).map_err(crate::error)?;
            }
            Report::Finished {
                id,
                produced,
                publication,
                messages,
                result,
            } => {
                let failure = result.as_ref().err().cloned();
                if self.resizing == Some(id) {
                    self.resizing = None;
                }
                if self.demand == Some(id) {
                    self.demand = None;
                }
                if !self
                    .jobs
                    .stamped(id, (result, Some(produced)))
                    .map_err(crate::error)?
                {
                    return Ok(());
                }
                self.pending -= 1;
                if let Some(fault) = failure {
                    self.errors.push(fault);
                }
                observe::call("content.install", || self.display.install(*publication))?;
                self.messages.extend(messages);
            }
        }
        Ok(())
    }
    fn fail(&mut self, reason: &str) {
        self.fault = Some(reason.into());
        self.errors.push(reason.into());
        self.jobs.close(reason);
        if self.display.state() == State::Loading {
            self.display.publication.state = State::Failed;
        }
        self.sender = None;
        self.receiver = None;
        self.closed.store(true, Ordering::Release);
        self.gate.close();
    }
    pub fn job(&self, id: u64) -> Option<&Job> {
        self.jobs.get(id)
    }
    pub fn messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.messages)
    }
    pub fn errors(&mut self) -> Vec<String> {
        let mut errors = std::mem::take(&mut self.errors);
        errors.append(&mut self.diagnostics);
        errors
    }
    pub fn close(&mut self) {
        self.jobs.close("content closed before completion");
        self.closed.store(true, Ordering::Release);
        self.gate.close();
        self.sender = None;
        self.receiver = None;
        self.messages.clear();
        self.errors.clear();
        self.diagnostics.clear();
        self.display.close();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.close();
    }
}
