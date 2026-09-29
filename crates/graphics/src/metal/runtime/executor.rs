use crate::metal::{Metal, Work, error};
use crate::shared::{Backend, Fault, Image, Pass, State};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

pub struct Executor {
    sender: SyncSender<Request>,
}

pub struct Execution {
    receiver: Receiver<Result<Transfer, Fault>>,
    artifact: Option<Result<Transfer, Fault>>,
}

enum Request {
    Submit(Pass, SyncSender<Result<Transfer, Fault>>),
    Read(Transfer, SyncSender<Result<Image, Fault>>),
    Wake,
}

#[derive(Clone)]
struct Transfer(Work);

struct Pending {
    work: Work,
    sender: SyncSender<Result<Transfer, Fault>>,
}

// SAFETY: Work crosses threads only after Metal reports its command complete;
// the receiving side reads retained immutable objects and never encodes with them.
unsafe impl Send for Transfer {}

impl Executor {
    pub fn new(notify: shared::Notify) -> Result<Self, Fault> {
        let (sender, receiver) = mpsc::sync_channel(16);
        let events = sender.clone();
        std::thread::Builder::new()
            .name("metal-compute".into())
            .spawn(move || worker(receiver, events, notify))
            .map_err(error)?;
        Ok(Self { sender })
    }
}

impl Execution {
    pub(crate) fn artifact(&self) -> Result<&Work, Fault> {
        match self.artifact.as_ref() {
            Some(Ok(artifact)) => Ok(&artifact.0),
            Some(Err(fault)) => Err(fault.clone()),
            None => Err(Fault("Metal execution is pending".into())),
        }
    }

    fn poll(&mut self) -> Result<State, Fault> {
        if let Some(result) = &self.artifact {
            return result.as_ref().map_or_else(
                |fault| Ok(State::Failed(fault.clone())),
                |_| Ok(State::Complete),
            );
        }
        match self.receiver.try_recv() {
            Ok(result) => {
                let state = result
                    .as_ref()
                    .map_or_else(|fault| State::Failed(fault.clone()), |_| State::Complete);
                self.artifact = Some(result);
                Ok(state)
            }
            Err(TryRecvError::Empty) => Ok(State::Pending),
            Err(TryRecvError::Disconnected) => {
                let fault = Fault("Metal executor stopped before completing work".into());
                self.artifact = Some(Err(fault.clone()));
                Ok(State::Failed(fault))
            }
        }
    }
}

impl Backend for Executor {
    type Work = Execution;

    fn submit(&mut self, pass: &Pass) -> Result<Execution, Fault> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Request::Submit(pass.clone(), sender))
            .map_err(error)?;
        Ok(Execution {
            receiver,
            artifact: None,
        })
    }

    fn poll(&mut self, work: &mut Execution) -> Result<State, Fault> {
        work.poll()
    }

    fn read(&mut self, work: &Execution) -> Result<Image, Fault> {
        let artifact = Transfer(work.artifact()?.clone());
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .send(Request::Read(artifact, sender))
            .map_err(error)?;
        receiver.recv().map_err(error)?
    }
}

fn worker(receiver: Receiver<Request>, events: SyncSender<Request>, notify: shared::Notify) {
    let wake = shared::Notify::new(move || {
        let _ = events.try_send(Request::Wake);
    });
    let initialized = Metal::new().map(|metal| metal.notified(wake));
    notify.notify();
    match initialized {
        Ok(metal) => run(metal, receiver, notify),
        Err(fault) => failed(receiver, fault, notify),
    }
}

fn run(mut metal: Metal, receiver: Receiver<Request>, notify: shared::Notify) {
    eprintln!("[renderer:metal] {}", metal.name());
    let mut pending = Vec::new();
    loop {
        let Ok(request) = receiver.recv() else {
            abandon(pending);
            break;
        };
        let mut completed = execute(&mut metal, &mut pending, request);
        while let Ok(request) = receiver.try_recv() {
            completed |= execute(&mut metal, &mut pending, request);
        }
        completed |= poll(&mut metal, &mut pending);
        if completed {
            notify.notify();
        }
    }
}

fn execute(metal: &mut Metal, pending: &mut Vec<Pending>, request: Request) -> bool {
    match request {
        Request::Submit(pass, sender) => match metal.submit(&pass) {
            Ok(work) => pending.push(Pending { work, sender }),
            Err(fault) => {
                let _ = sender.send(Err(fault));
                return true;
            }
        },
        Request::Read(work, sender) => {
            let _ = sender.send(metal.read(&work.0));
        }
        Request::Wake => (),
    }
    false
}

fn poll(metal: &mut Metal, pending: &mut Vec<Pending>) -> bool {
    let mut completed = false;
    pending.retain_mut(|task| match metal.poll(&mut task.work) {
        Ok(State::Pending) => true,
        Ok(State::Complete) => {
            let _ = task.sender.send(Ok(Transfer(task.work.clone())));
            completed = true;
            false
        }
        Ok(State::Failed(fault)) | Err(fault) => {
            let _ = task.sender.send(Err(fault));
            completed = true;
            false
        }
    });
    completed
}

fn abandon(pending: Vec<Pending>) {
    let fault = Fault("Metal executor stopped before completing work".into());
    for task in pending {
        let _ = task.sender.send(Err(fault.clone()));
    }
}

fn failed(receiver: Receiver<Request>, fault: Fault, notify: shared::Notify) {
    while let Ok(request) = receiver.recv() {
        match request {
            Request::Submit(_, sender) => {
                let _ = sender.send(Err(fault.clone()));
            }
            Request::Read(_, sender) => {
                let _ = sender.send(Err(fault.clone()));
            }
            Request::Wake => continue,
        }
        notify.notify();
    }
}
