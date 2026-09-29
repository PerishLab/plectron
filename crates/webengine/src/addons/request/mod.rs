mod response;
mod scope;
use crate::Fault;
use crate::resource::{Cancellation, Request, Response, Transport, Url};
use jsengine::{Deferred, Engine, Value};
use serde::{Deserialize, Serialize};
use shared::budget::Budget;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    mpsc::{self, Receiver, TryRecvError},
};

static WORKERS: Budget = Budget::new(32);

#[derive(Clone, Copy)]
pub(super) enum Body {
    Text,
    Json,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Requests {
    pub pending: usize,
    pub started: usize,
    pub completed: usize,
    pub cancelled: usize,
    pub failed: usize,
}
#[derive(Default)]
pub(super) struct Fetch {
    base: Option<Url>,
    transport: Option<Arc<dyn Transport>>,
    pending: BTreeMap<usize, Pending>,
    closed: BTreeSet<usize>,
    scopes: usize,
    report: Requests,
    pub bodies: Vec<Option<Vec<u8>>>,
}
struct Pending {
    context: Option<observe::Context>,
    scope: usize,
    deferred: Deferred,
    cancellation: Cancellation,
    receiver: Receiver<observe::Envelope<Result<Response, Fault>>>,
}
impl Fetch {
    pub(super) fn roots(&self, visit: &mut dyn FnMut(&Value)) {
        for pending in self.pending.values() {
            visit(&pending.deferred.promise);
            visit(&pending.deferred.resolve);
            visit(&pending.deferred.reject);
        }
    }

    pub fn setup(&mut self, url: Url, transport: Option<Arc<dyn Transport>>) {
        self.base = Some(url);
        self.transport = transport;
    }
    pub fn inspect(&self) -> Requests {
        Requests {
            pending: self.pending.len(),
            ..self.report.clone()
        }
    }
    fn start(&mut self, scope: usize, source: &str, engine: &mut Engine) -> Result<Value, Fault> {
        let deferred = engine.deferred()?;
        if self.closed.contains(&scope) {
            self.pending(
                scope,
                deferred.clone(),
                Err(Fault("request scope cancelled".into())),
            )?;
            return Ok(deferred.promise);
        }
        let result = self.prepare(source);
        let (request, transport) = match result {
            Ok(result) => result,
            Err(fault) => {
                self.pending(scope, deferred.clone(), Err(fault))?;
                return Ok(deferred.promise);
            }
        };
        if self.pending.len() >= 32 || self.report.started >= 4096 {
            return Err(Fault("request budget exhausted".into()));
        }
        let Some(permit) = WORKERS.acquire() else {
            self.pending(
                scope,
                deferred.clone(),
                Err(Fault("request transport capacity exhausted".into())),
            )?;
            return Ok(deferred.promise);
        };
        let cancellation = Cancellation::default();
        let signal = cancellation.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let context = observe::Transfer::capture();
        let transport = (context, transport);
        std::thread::Builder::new()
            .name("plectron-request".into())
            .spawn(move || {
                let _permit = permit;
                let (context, transport) = transport;
                let mut scope = context.enter("resource.transport");
                let result = signal
                    .check()
                    .and_then(|()| transport.send(&request, &signal))
                    .and_then(|response| {
                        signal.check()?;
                        if response.body.len() > 65_536 {
                            return Err(Fault("response byte budget exhausted".into()));
                        }
                        Ok(response)
                    });
                let success = result.is_ok();
                let sent = sender.send(result.into()).is_ok();
                scope.finish(success && sent);
            })
            .map_err(crate::error)?;
        self.report.started += 1;
        self.pending.insert(
            self.report.started,
            Pending {
                context: observe::current(),
                scope,
                deferred: deferred.clone(),
                cancellation,
                receiver,
            },
        );
        Ok(deferred.promise)
    }
    fn prepare(&self, source: &str) -> Result<(Request, Arc<dyn Transport>), Fault> {
        let base = self
            .base
            .as_ref()
            .ok_or_else(|| Fault("resource context is absent".into()))?;
        let url = base.join(source).map_err(crate::error)?;
        let url = crate::resource::address(url.as_str())?;
        let transport = self
            .transport
            .clone()
            .ok_or_else(|| Fault("dynamic resource transport is absent".into()))?;
        Ok((Request::new(url, crate::resource::Kind::Text), transport))
    }
    fn pending(
        &mut self,
        scope: usize,
        deferred: Deferred,
        result: Result<Response, Fault>,
    ) -> Result<(), Fault> {
        if self.pending.len() >= 32 || self.report.started >= 4096 {
            return Err(Fault("request budget exhausted".into()));
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(sender.send(result.into()).is_ok(), "new request receiver");
        self.report.started += 1;
        self.pending.insert(
            self.report.started,
            Pending {
                context: observe::current(),
                scope,
                deferred,
                cancellation: Cancellation::default(),
                receiver,
            },
        );
        Ok(())
    }
    pub fn ready(&self) -> bool {
        !self.pending.is_empty()
    }
}
impl Drop for Fetch {
    fn drop(&mut self) {
        for pending in self.pending.values() {
            pending.cancellation.cancel();
        }
    }
}
impl super::Web {
    pub fn requests(&self) -> Requests {
        self.fetch.inspect()
    }
    pub fn transport(&mut self, url: Url, transport: Option<Arc<dyn Transport>>) {
        self.fetch.setup(url, transport);
    }
    pub fn pending(&self) -> bool {
        self.fetch.ready()
    }
    pub fn poll(&mut self, engine: &mut Engine) -> Result<usize, Fault> {
        let ready: Vec<_> = self
            .fetch
            .pending
            .iter()
            .filter_map(|(id, pending)| match pending.receiver.try_recv() {
                Ok(result) => Some((*id, result)),
                Err(TryRecvError::Disconnected) => {
                    let mut scope =
                        observe::resume(pending.context.as_ref(), "resource.disconnect");
                    let result = Some((
                        *id,
                        observe::Envelope::new(Err(Fault("request transport disconnected".into()))),
                    ));
                    scope.finish(false);
                    result
                }
                Err(TryRecvError::Empty) => None,
            })
            .collect();
        let count = ready.len();
        for (id, envelope) in ready {
            let (result, mut scope) = envelope.receive("resource.complete");
            let pending = self.fetch.pending.remove(&id).expect("ready request");
            let completed = self.completed(pending, result, engine);
            scope.finish(completed.is_ok());
            completed?;
        }
        Ok(count)
    }
    fn completed(
        &mut self,
        pending: Pending,
        result: Result<Response, Fault>,
        engine: &mut Engine,
    ) -> Result<(), Fault> {
        let (callback, value) = match result {
            Ok(response) => {
                self.fetch.report.completed += 1;
                (pending.deferred.resolve, self.response(engine, response)?)
            }
            Err(fault) => {
                self.fetch.report.failed += 1;
                (
                    pending.deferred.reject,
                    engine.exception(jsengine::Error::Type, Some(fault.0))?,
                )
            }
        };
        engine
            .call(callback, vec![value], self)
            .map_err(|failure| engine.diagnostic(&failure))?;
        Ok(())
    }
}
