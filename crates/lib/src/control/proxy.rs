use crate::contract::{Fault, Signal, Unit, Window};
use crate::platform::Host;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver, SyncSender},
};

type Reply<T> = SyncSender<Result<T, Fault>>;
pub struct Envelope {
    context: observe::Transfer,
    request: Request,
}
impl Envelope {
    pub fn apply(self, host: &mut impl Host) {
        let mut scope = self.context.enter("platform.request");
        let success = self.request.apply(host);
        scope.finish(success);
    }
}
pub enum Request {
    Window(Window, Reply<Unit>),
    Title(Unit, String, Reply<()>),
    Fill(Unit, u32, Reply<()>),
    Close(Unit, Reply<()>),
    Check(Unit, Reply<()>),
    Log(String, Reply<()>),
    Serve(String, Reply<u64>),
    Scheme(u64, String, Reply<()>),
    Stop(u64, Reply<()>),
    Attach(Unit, String, Reply<()>),
    Message(Unit, String, Reply<()>),
    Input(Signal, Reply<bool>),
    Finish(Reply<()>),
}
impl Request {
    pub fn apply(self, host: &mut impl Host) -> bool {
        match self {
            Self::Window(options, reply) => respond(reply, host.window(options)),
            Self::Title(unit, text, reply) => respond(reply, host.title(unit, &text)),
            Self::Fill(unit, color, reply) => respond(reply, host.fill(unit, color)),
            Self::Close(unit, reply) => respond(reply, host.close(unit)),
            Self::Check(unit, reply) => respond(reply, host.check(unit)),
            Self::Log(text, reply) => respond(reply, host.log(&text)),
            Self::Serve(root, reply) => respond(reply, host.serve(&root)),
            Self::Scheme(service, origin, reply) => respond(reply, host.scheme(service, &origin)),
            Self::Stop(service, reply) => respond(reply, host.stop(service)),
            Self::Attach(unit, url, reply) => respond(reply, host.attach(unit, &url)),
            Self::Message(unit, text, reply) => respond(reply, host.message(unit, &text)),
            Self::Input(signal, reply) => respond(reply, host.input(&signal)),
            Self::Finish(reply) => respond(reply, host.finish()),
        }
    }
}
fn respond<T>(reply: Reply<T>, result: Result<T, Fault>) -> bool {
    let success = result.is_ok();
    reply.send(result).is_ok() && success
}

pub struct Proxy {
    sender: SyncSender<Envelope>,
    waiting: Arc<AtomicBool>,
    notify: Option<shared::Notify>,
}
impl Proxy {
    pub fn channel() -> (Self, Receiver<Envelope>) {
        let (sender, receiver) = mpsc::sync_channel(1);
        (
            Self {
                sender,
                waiting: Arc::new(AtomicBool::new(false)),
                notify: None,
            },
            receiver,
        )
    }
    pub fn notified(mut self, notify: shared::Notify) -> Self {
        self.notify = Some(notify);
        self
    }
    pub(super) fn waiting(&self) -> Arc<AtomicBool> {
        self.waiting.clone()
    }
    fn call<T>(&self, request: impl FnOnce(Reply<T>) -> Request) -> Result<T, Fault> {
        let (reply, receiver) = mpsc::sync_channel(1);
        self.waiting.store(true, Ordering::Release);
        let result = self.exchange(request(reply), receiver);
        self.waiting.store(false, Ordering::Release);
        result
    }
    fn exchange<T>(
        &self,
        request: Request,
        receiver: Receiver<Result<T, Fault>>,
    ) -> Result<T, Fault> {
        self.sender
            .send(Envelope {
                request,
                context: observe::Transfer::capture(),
            })
            .map_err(|_| Fault("platform disconnected".into()))?;
        if let Some(notify) = &self.notify {
            notify.notify();
        }
        receiver
            .recv()
            .map_err(|_| Fault("platform disconnected".into()))?
    }
}
impl Host for Proxy {
    fn window(&mut self, options: Window) -> Result<Unit, Fault> {
        self.call(|reply| Request::Window(options, reply))
    }
    fn title(&mut self, unit: Unit, title: &str) -> Result<(), Fault> {
        self.call(|reply| Request::Title(unit, title.into(), reply))
    }
    fn fill(&mut self, unit: Unit, color: u32) -> Result<(), Fault> {
        self.call(|reply| Request::Fill(unit, color, reply))
    }
    fn close(&mut self, unit: Unit) -> Result<(), Fault> {
        self.call(|reply| Request::Close(unit, reply))
    }
    fn check(&self, unit: Unit) -> Result<(), Fault> {
        self.call(|reply| Request::Check(unit, reply))
    }
    fn log(&mut self, text: &str) -> Result<(), Fault> {
        self.call(|reply| Request::Log(text.into(), reply))
    }
    fn serve(&mut self, root: &str) -> Result<u64, Fault> {
        self.call(|reply| Request::Serve(root.into(), reply))
    }
    fn scheme(&mut self, service: u64, origin: &str) -> Result<(), Fault> {
        self.call(|reply| Request::Scheme(service, origin.into(), reply))
    }
    fn stop(&mut self, service: u64) -> Result<(), Fault> {
        self.call(|reply| Request::Stop(service, reply))
    }
    fn attach(&mut self, unit: Unit, url: &str) -> Result<(), Fault> {
        self.call(|reply| Request::Attach(unit, url.into(), reply))
    }
    fn message(&mut self, unit: Unit, text: &str) -> Result<(), Fault> {
        self.call(|reply| Request::Message(unit, text.into(), reply))
    }
    fn input(&mut self, signal: &Signal) -> Result<bool, Fault> {
        self.call(|reply| Request::Input(signal.clone(), reply))
    }
    fn finish(&mut self) -> Result<(), Fault> {
        self.call(Request::Finish)
    }
}
