use super::protocol::Report;
use shared::Notify;
use std::sync::mpsc::SyncSender;

pub(super) struct Publisher {
    sender: SyncSender<observe::Envelope<Report>>,
    notify: Option<Notify>,
}
impl Publisher {
    pub fn new(sender: SyncSender<observe::Envelope<Report>>, notify: Option<Notify>) -> Self {
        Self { sender, notify }
    }
    pub fn send(&self, report: Report) -> Result<(), ()> {
        self.sender.send(report.into()).map_err(|_| ())?;
        if let Some(notify) = &self.notify {
            notify.notify();
        }
        Ok(())
    }
}
