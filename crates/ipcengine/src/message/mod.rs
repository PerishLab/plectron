mod codec;
mod io;
mod server;
use crate::Fault;
pub use codec::Connection;
pub use server::Server;
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct Envelope<Q, R> {
    pub request: Q,
    pub reply: mpsc::SyncSender<R>,
    deadline: Instant,
}
impl<Q, R> Envelope<Q, R> {
    pub fn expired(&self) -> bool {
        Instant::now() >= self.deadline
    }
}
pub fn ask<Q, R>(
    sender: &mpsc::SyncSender<Envelope<Q, R>>,
    request: Q,
    notify: Option<&shared::Notify>,
) -> Result<R, Fault> {
    let (reply, receiver) = mpsc::sync_channel(1);
    sender
        .try_send(Envelope {
            request,
            reply,
            deadline: Instant::now() + Duration::from_secs(2),
        })
        .map_err(|error| Fault(error.to_string()))?;
    if let Some(notify) = notify {
        notify.notify();
    }
    receiver
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| Fault("request handler did not reply; execution outcome is unknown".into()))
}
