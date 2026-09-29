use crate::{Context, Guard, Kind, context};

#[derive(Clone)]
pub struct Transfer(Option<Origin>);
#[derive(Clone)]
struct Origin {
    context: Context,
    captured: u64,
    thread: u64,
}
impl Transfer {
    pub fn capture() -> Self {
        Self(crate::current().map(|context| Origin {
            captured: context.session.0.elapsed(),
            thread: context::thread(),
            context,
        }))
    }
    pub fn enter(&self, name: impl Into<String>) -> Guard {
        match &self.0 {
            Some(origin) => origin.context.open(
                name.into(),
                Kind::Receive {
                    captured: origin.captured,
                    thread: origin.thread,
                },
            ),
            None => crate::resume(None, name),
        }
    }
}

pub struct Envelope<T> {
    value: T,
    transfer: Transfer,
}
impl<T> Envelope<T> {
    pub fn new(value: T) -> Self {
        Self {
            value,
            transfer: Transfer::capture(),
        }
    }
    pub fn receive(self, name: impl Into<String>) -> (T, Guard) {
        (self.value, self.transfer.enter(name))
    }

    pub fn peek(&self) -> &T {
        &self.value
    }
}
impl<T> From<T> for Envelope<T> {
    fn from(value: T) -> Self {
        Self::new(value)
    }
}
