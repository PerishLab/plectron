use super::swapchain::Swapchain;
use super::telemetry::Telemetry;
use crate::d3d12::Work;
use crate::d3d12::ffi::Handle;
use crate::shared::{Extent, Fault, State, Visibility as Evidence};
use shared::budget::Budget;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};
use std::thread::JoinHandle;
use std::time::Instant;

static WORKERS: Budget = Budget::new(8);

enum Command {
    Present(Work),
    Resize(Extent, SyncSender<Result<(), Fault>>),
}
// SAFETY: Work holds an Arc<Receipt> with completed GPU texture and thread-safe queue/device refs.
unsafe impl Send for Command {}

struct Outcome {
    result: Result<(), Fault>,
    completed: Instant,
    visibility: Option<Telemetry>,
}

pub(crate) struct Worker {
    sender: Option<SyncSender<Command>>,
    receiver: Receiver<Outcome>,
    thread: Option<JoinHandle<()>>,
    cancel: Arc<AtomicBool>,
    pending: bool,
    completed: Option<Instant>,
    visibility: Option<Telemetry>,
}

impl Worker {
    pub fn new(
        handle: Handle,
        work: &Work,
        extent: Extent,
        notify: Option<shared::Notify>,
    ) -> Result<Self, Fault> {
        let permit = WORKERS
            .acquire()
            .ok_or_else(|| Fault("presentation worker budget exhausted".into()))?;
        let swapchain = Swapchain::new(handle, work, extent)?;
        let (sender, jobs) = mpsc::sync_channel::<Command>(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let cancel = Arc::new(AtomicBool::new(false));
        let stop = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("d3d12-present".into())
            .spawn(move || {
                let _permit = permit;
                run(swapchain, jobs, results, (stop, notify));
            })
            .map_err(crate::d3d12::error)?;
        Ok(Self {
            sender: Some(sender),
            receiver,
            thread: Some(thread),
            cancel,
            pending: false,
            completed: None,
            visibility: None,
        })
    }

    pub fn present(&mut self, work: &Work) -> Result<(), Fault> {
        if self.pending {
            return Err(Fault("presentation already pending".into()));
        }
        if work.receipt.signal.state() != State::Complete {
            return Err(Fault("presentation requires completed rendering".into()));
        }
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| Fault("presentation worker closed".into()))?;
        sender
            .try_send(Command::Present(work.clone()))
            .map_err(crate::d3d12::error)?;
        self.pending = true;
        self.completed = None;
        self.visibility = None;
        Ok(())
    }

    pub fn resize(&mut self, extent: Extent) -> Result<(), Fault> {
        if self.pending {
            return Err(Fault("cannot resize while presentation is pending".into()));
        }
        self.visibility = None;
        let sender = self
            .sender
            .as_ref()
            .ok_or_else(|| Fault("presentation worker closed".into()))?;
        let (reply, receiver) = mpsc::sync_channel(1);
        sender
            .send(Command::Resize(extent, reply))
            .map_err(crate::d3d12::error)?;
        receiver.recv().map_err(crate::d3d12::error)?
    }

    pub fn completed(&self) -> Option<Instant> {
        self.completed
    }

    pub fn visibility(&self) -> Evidence {
        self.visibility
            .as_ref()
            .map_or(Evidence::Unobserved, Telemetry::evidence)
    }

    pub fn poll(&mut self) -> Result<State, Fault> {
        if !self.pending {
            return Err(Fault("presentation is absent".into()));
        }
        match self.receiver.try_recv() {
            Ok(outcome) => {
                self.completed = Some(outcome.completed);
                self.visibility = outcome.visibility;
                self.pending = false;
                outcome.result.map(|()| State::Complete)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(State::Pending),
            Err(fault) => Err(crate::d3d12::error(fault)),
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.visibility = None;
        self.sender = None;
        while self.receiver.try_recv().is_ok() {}
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        while self.receiver.try_recv().is_ok() {}
    }
}

fn run(
    mut swapchain: Swapchain,
    jobs: Receiver<Command>,
    results: SyncSender<Outcome>,
    options: (Arc<AtomicBool>, Option<shared::Notify>),
) {
    let (stop, notify) = options;
    while let Ok(command) = jobs.recv() {
        if stop.load(Ordering::Acquire)
            || !dispatch(&mut swapchain, command, &results, (&stop, notify.as_ref()))
        {
            break;
        }
    }
    swapchain.drain();
}

fn dispatch(
    swapchain: &mut Swapchain,
    command: Command,
    results: &SyncSender<Outcome>,
    options: (&AtomicBool, Option<&shared::Notify>),
) -> bool {
    let (stop, notify) = options;
    match command {
        Command::Present(work) => {
            let outcome = match swapchain.encode(&work, stop) {
                Ok((completed, visibility)) => Outcome {
                    result: Ok(()),
                    completed,
                    visibility,
                },
                Err(fault) => Outcome {
                    result: Err(fault),
                    completed: Instant::now(),
                    visibility: None,
                },
            };
            if stop.load(Ordering::Acquire) || results.try_send(outcome).is_err() {
                return false;
            }
            if let Some(notify) = notify {
                notify.notify();
            }
            true
        }
        Command::Resize(extent, reply) => {
            let _ = reply.send(swapchain.resize(extent));
            true
        }
    }
}
