use super::addon::{Addon, Dialog, Surface};
use super::broker::Broker;
use super::host::Host;
use platform::shared::{Backend, Progress, Region, Unit};
use webengine::{Action, Fault, State, Worker};

pub(crate) fn assemble(
    dialog: Option<Box<dyn Dialog>>,
    addons: Vec<Box<dyn Addon>>,
) -> Result<Broker, Fault> {
    let mut broker = Broker::default();
    broker.register(Box::new(Host::new(dialog)))?;
    for addon in addons {
        broker.register(addon)?;
    }
    Ok(broker)
}

pub(crate) struct Adapter<'a, B> {
    pub backend: &'a mut B,
    pub unit: Unit,
    pub worker: &'a mut Worker,
    pub closed: &'a mut bool,
}

impl<B: Backend<Fault>> Surface for Adapter<'_, B> {
    fn title(&mut self, title: &str) -> Result<(), String> {
        self.backend
            .title(self.unit, title)
            .map_err(|error| error.0)
    }

    fn drag(&mut self, regions: &[Region]) -> Result<(), String> {
        self.backend
            .drag(self.unit, regions)
            .map_err(|error| error.0)
    }

    fn progress(&mut self, progress: Progress) -> Result<(), String> {
        self.backend
            .progress(self.unit, progress)
            .map_err(|error| error.0)
    }

    fn close(&mut self) -> Result<(), String> {
        if !*self.closed {
            *self.closed = true;
            self.worker.close();
            self.backend.close(self.unit).map_err(|error| error.0)?;
        }
        Ok(())
    }
}

pub(crate) fn drain<B: Backend<Fault>>(
    broker: &mut Broker,
    adapter: &mut Adapter<'_, B>,
) -> Result<(), Fault> {
    for error in adapter.worker.errors() {
        adapter.backend.log(&error)?;
    }
    let incoming = adapter.worker.messages();
    for message in incoming {
        if *adapter.closed {
            return Ok(());
        }
        let _ = broker.receive(&message, adapter);
    }
    if *adapter.closed {
        return Ok(());
    }
    broker.poll(adapter);
    if *adapter.closed || adapter.worker.state() != State::Ready {
        return Ok(());
    }
    while let Some(reply) = broker.peek() {
        match adapter.worker.submit(Action::Receive(reply.to_owned())) {
            Ok(_) => {
                let _ = broker.pop();
            }
            Err(Fault(reason)) if reason == "content mailbox unavailable" => break,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}
