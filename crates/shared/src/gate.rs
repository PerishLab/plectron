use std::sync::mpsc::{self, Receiver, SyncSender};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    Held,
    Closed,
    Cancelled,
}
impl std::fmt::Display for Fault {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        out.write_str(match self {
            Self::Held => "gate already held",
            Self::Closed => "gate closed",
            Self::Cancelled => "hold cancelled",
        })
    }
}
impl std::error::Error for Fault {}
#[derive(Default)]
enum State {
    #[default]
    Open,
    Held(SyncSender<()>),
    Closed,
}
#[derive(Default)]
pub struct Gate {
    state: State,
}
pub struct Ticket {
    receiver: Receiver<()>,
}
impl Gate {
    pub fn hold(&mut self) -> Result<Ticket, Fault> {
        match self.state {
            State::Held(_) => return Err(Fault::Held),
            State::Closed => return Err(Fault::Closed),
            State::Open => (),
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        self.state = State::Held(sender);
        Ok(Ticket { receiver })
    }
    pub fn release(&mut self) -> Result<(), Fault> {
        if matches!(self.state, State::Closed) {
            return Err(Fault::Closed);
        }
        if let State::Held(sender) = std::mem::take(&mut self.state) {
            let _ = sender.send(());
        }
        Ok(())
    }
    pub fn close(&mut self) {
        self.state = State::Closed;
    }
}
impl Ticket {
    pub fn wait(self) -> Result<(), Fault> {
        self.receiver.recv().map_err(|_| Fault::Cancelled)
    }
}
