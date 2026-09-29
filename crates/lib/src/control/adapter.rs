use super::{Snapshot, Worker};
use crate::addons::control::{Control, Dispatch};
use crate::contract::{Fault, Signal};
use crate::platform::{Backend, Evaluation, Host, Platform};
use crate::rendering::Renderer;
use crate::runtime::Runtime;

pub(crate) enum Adapter<B, R> {
    Local(Box<Runtime<Control<Platform<B, R>>>>),
    Threaded {
        platform: Box<Platform<B, R>>,
        worker: Box<Worker>,
    },
}
impl<B: Backend, R: Renderer> Adapter<B, R> {
    pub fn host(&mut self) -> &mut Platform<B, R> {
        match self {
            Self::Local(runtime) => &mut runtime.addons().host,
            Self::Threaded { platform, .. } => platform,
        }
    }
    pub fn spawn(
        mut platform: Platform<B, R>,
        source: &str,
        notify: Option<shared::Notify>,
    ) -> Result<Self, Fault> {
        platform.notify(notify.clone());
        Ok(Self::Threaded {
            platform: Box::new(platform),
            worker: Box::new(Worker::spawn(source, notify)?),
        })
    }
    pub fn evaluate(&mut self, source: &str) -> Result<Evaluation, Fault> {
        match self {
            Self::Local(runtime) => {
                let display = runtime
                    .execute(source)
                    .map_err(|failure| runtime.diagnostic(&failure))?
                    .text();
                runtime.checkpoint()?;
                runtime.addons().host.finish()?;
                Ok(Evaluation::Value(display))
            }
            Self::Threaded { worker, .. } => worker.evaluate(source).map(Evaluation::Queued),
        }
    }
    pub fn dispatch(&mut self, signal: Signal) -> Result<(), Fault> {
        match self {
            Self::Local(runtime) => runtime.dispatch(signal),
            Self::Threaded { worker, .. } => worker.dispatch(signal).map(|_| ()),
        }
    }
    pub fn busy(&self) -> bool {
        matches!(self, Self::Threaded { worker, .. } if worker.busy())
    }
    pub fn available(&self) -> bool {
        match self {
            Self::Local(_) => true,
            Self::Threaded { worker, .. } => worker.available(),
        }
    }
    pub fn poll(&mut self) -> Result<Option<String>, Fault> {
        match self {
            Self::Local(_) => Ok(None),
            Self::Threaded { platform, worker } => {
                worker.poll(platform.as_mut());
                worker.startup()?;
                Ok(worker.failure())
            }
        }
    }
    pub fn job(&mut self, id: u64) -> Option<&super::protocol::Job> {
        self.worker().ok()?.job(id)
    }
    pub fn inspect(&mut self) -> Result<Snapshot, Fault> {
        Ok(self.worker()?.inspect())
    }
    pub fn hold(&mut self) -> Result<u64, Fault> {
        self.worker()?.hold()
    }
    pub fn unhold(&mut self) -> Result<(), Fault> {
        self.worker()?.unhold()
    }
    fn worker(&mut self) -> Result<&mut Worker, Fault> {
        match self {
            Self::Threaded { worker, .. } => Ok(worker),
            Self::Local(_) => Err(Fault("local control has no worker".into())),
        }
    }
    pub fn close(&mut self) {
        if let Self::Threaded { worker, .. } = self {
            worker.close();
        }
    }
}
