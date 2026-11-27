use crate::d3d12::{D3d12, Work, error};
use crate::shared::{Backend, Fault, Image, Pass, State};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender, TryRecvError},
};

#[derive(Clone)]
pub struct Probe(Arc<AtomicBool>);

impl Probe {
    pub fn ready(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}

pub struct Executor {
    sender: SyncSender<Request>,
    probe: Probe,
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

// SAFETY: Work crosses threads only after D3D12 reports its fence complete;
// Receipt is Arc-backed and Send + Sync, and the receiving side never encodes onto D3d12.
unsafe impl Send for Transfer {}

impl Executor {
    pub fn new(notify: shared::Notify) -> Result<Self, Fault> {
        let (sender, receiver) = mpsc::sync_channel(16);
        let events = sender.clone();
        let ready = Arc::new(AtomicBool::new(false));
        let signal = ready.clone();
        std::thread::Builder::new()
            .name("d3d12-compute".into())
            .spawn(move || worker(receiver, events, notify, signal))
            .map_err(error)?;
        Ok(Self {
            sender,
            probe: Probe(ready),
        })
    }

    pub fn probe(&self) -> Probe {
        self.probe.clone()
    }
}

impl Execution {
    pub(crate) fn artifact(&self) -> Result<&Work, Fault> {
        match self.artifact.as_ref() {
            Some(Ok(artifact)) => Ok(&artifact.0),
            Some(Err(fault)) => Err(fault.clone()),
            None => Err(Fault("D3D12 execution is pending".into())),
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
                let fault = Fault("D3D12 executor stopped before completing work".into());
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

fn worker(
    receiver: Receiver<Request>,
    events: SyncSender<Request>,
    notify: shared::Notify,
    ready: Arc<AtomicBool>,
) {
    let wake = shared::Notify::new(move || {
        let _ = events.try_send(Request::Wake);
    });
    let initialized = D3d12::new().map(|device| device.notified(wake));
    ready.store(true, Ordering::Release);
    notify.notify();
    match initialized {
        Ok(device) => run(device, receiver, notify),
        Err(fault) => failed(receiver, fault, notify),
    }
}

fn run(mut device: D3d12, receiver: Receiver<Request>, notify: shared::Notify) {
    eprintln!("[renderer:d3d12] {}", device.name());
    let mut pending = Vec::new();
    loop {
        let Ok(request) = receiver.recv() else {
            abandon(pending);
            break;
        };
        let mut completed = execute(&mut device, &mut pending, request);
        while let Ok(request) = receiver.try_recv() {
            completed |= execute(&mut device, &mut pending, request);
        }
        completed |= poll(&mut device, &mut pending);
        if completed {
            notify.notify();
        }
    }
}

fn execute(device: &mut D3d12, pending: &mut Vec<Pending>, request: Request) -> bool {
    match request {
        Request::Submit(pass, sender) => match device.submit(&pass) {
            Ok(work) => pending.push(Pending { work, sender }),
            Err(fault) => {
                let _ = sender.send(Err(fault));
                return true;
            }
        },
        Request::Read(work, sender) => {
            let _ = sender.send(device.read(&work.0));
        }
        Request::Wake => (),
    }
    false
}

fn poll(device: &mut D3d12, pending: &mut Vec<Pending>) -> bool {
    let mut completed = false;
    pending.retain_mut(|task| match device.poll(&mut task.work) {
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
    let fault = Fault("D3D12 executor stopped before completing work".into());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::{Cpu, Extent, Frame, Item, Output, Rect, Scene, Visual};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{Duration, Instant};

    #[test]
    fn parity() {
        let extent = Extent {
            width: 64,
            height: 48,
        };
        let scene = Scene::new(
            extent,
            1,
            vec![Item::Fill {
                rect: Rect {
                    x: 8,
                    y: 6,
                    width: 40,
                    height: 28,
                },
                color: Some(0x336699),
            }],
        )
        .unwrap();
        let frame = Frame::new(scene, Visual::new(extent)).unwrap();
        let pass = frame.prepare(Output::new(extent, 1.0).unwrap()).unwrap();
        let woken = Arc::new(AtomicBool::new(false));
        let signal = woken.clone();
        let mut executor = Executor::new(shared::Notify::new(move || {
            signal.store(true, Ordering::Release);
        }))
        .unwrap();
        let mut work = executor.submit(&pass).unwrap();
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(10) {
            match executor.poll(&mut work).unwrap() {
                State::Pending => std::thread::sleep(Duration::from_millis(2)),
                State::Complete => break,
                State::Failed(fault) => panic!("executor failed: {fault}"),
            }
        }
        assert!(woken.load(Ordering::Acquire));
        let actual = executor.read(&work).unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        assert_eq!(actual, expected);
    }
}
