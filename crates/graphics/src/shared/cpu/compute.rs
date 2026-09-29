use crate::shared::{Backend, Cpu, Fault, Image, Pass, State};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

pub struct Compute {
    sender: SyncSender<Request>,
}

pub struct Task {
    receiver: Receiver<Result<Image, Fault>>,
    result: Option<Result<Image, Fault>>,
}

struct Request {
    pass: Pass,
    sender: SyncSender<Result<Image, Fault>>,
}

impl Compute {
    pub fn new(notify: shared::Notify) -> Result<Self, Fault> {
        let (sender, receiver) = mpsc::sync_channel::<Request>(16);
        std::thread::Builder::new()
            .name("cpu-compute".into())
            .spawn(move || worker(receiver, notify))
            .map_err(|fault| Fault(fault.to_string()))?;
        Ok(Self { sender })
    }

    pub fn submit(&self, pass: Pass) -> Result<Task, Fault> {
        let (sender, receiver) = mpsc::sync_channel(1);
        self.sender
            .try_send(Request { pass, sender })
            .map_err(|fault| Fault(fault.to_string()))?;
        Ok(Task {
            receiver,
            result: None,
        })
    }
}

impl Task {
    pub fn poll(&mut self) -> State {
        if self.result.is_none() {
            self.result = match self.receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => {
                    Some(Err(Fault("CPU compute worker stopped".into())))
                }
            };
        }
        match &self.result {
            Some(Ok(_)) => State::Complete,
            Some(Err(fault)) => State::Failed(fault.clone()),
            None => State::Pending,
        }
    }

    pub fn take(&mut self) -> Result<Option<Image>, Fault> {
        match self.poll() {
            State::Pending => Ok(None),
            State::Complete => self.result.take().expect("completed result").map(Some),
            State::Failed(_) => Err(self.result.take().expect("failed result").unwrap_err()),
        }
    }
}

fn worker(receiver: Receiver<Request>, notify: shared::Notify) {
    let mut cpu = Cpu;
    while let Ok(request) = receiver.recv() {
        let result = cpu.submit(&request.pass);
        let _ = request.sender.send(result);
        notify.notify();
    }
}
