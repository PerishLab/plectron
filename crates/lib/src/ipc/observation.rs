use super::Reply;
use crate::contract::Fault;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Command {
    Start { options: observe::Options },
    Status,
    Stop { id: u64 },
    Export { id: u64 },
    Discard { id: u64 },
}

#[derive(Default)]
pub(super) struct Recorder(Option<observe::Session>);
impl Recorder {
    pub fn scope(&self, name: &'static str) -> Option<observe::Guard> {
        self.0
            .as_ref()
            .filter(|session| !session.status().stopped)
            .map(|session| session.scope(name))
    }
    pub fn command(&mut self, command: Command) -> Result<Reply, Fault> {
        match command {
            Command::Start { options } => {
                if self.0.is_some() {
                    return Err(Fault(
                        "discard the previous observation before starting".into(),
                    ));
                }
                self.0 = Some(observe::Session::new(options));
            }
            Command::Status => (),
            Command::Stop { id } => self.session(id)?.stop(),
            Command::Export { id } => {
                return Ok(Reply::Observation {
                    report: Box::new(self.session(id)?.inspect()),
                });
            }
            Command::Discard { id } => {
                let status = self.session(id)?.status();
                if !status.stopped || status.open != 0 {
                    return Err(Fault(
                        "observation must be stopped and drained before discard".into(),
                    ));
                }
                self.0 = None;
            }
        }
        Ok(Reply::Observing {
            status: self.0.as_ref().map(observe::Session::status),
        })
    }
    fn session(&self, id: u64) -> Result<&observe::Session, Fault> {
        self.0
            .as_ref()
            .filter(|session| session.status().session == id)
            .ok_or_else(|| Fault("unknown observation session".into()))
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        if let Some(session) = &self.0 {
            session.stop();
        }
    }
}
