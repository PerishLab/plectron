use super::{Backend, Event, Fault, Renderer, Reply, Session, Signal};
impl<B: Backend, R: Renderer> Session<B, R> {
    pub(super) fn input(
        &mut self,
        id: u64,
        input: webengine::keyboard::Input,
    ) -> Result<Reply, Fault> {
        input.validate().map_err(|fault| Fault(fault.to_string()))?;
        self.enqueue(id, Event::Input(input))
    }
    pub(super) fn edit(&mut self, id: u64, edit: webengine::Edit) -> Result<Reply, Fault> {
        edit.validate().map_err(|fault| Fault(fault.0))?;
        self.enqueue(id, Event::Edit(edit))
    }
    pub(super) fn composition(
        &mut self,
        id: u64,
        input: webengine::ime::Input,
    ) -> Result<Reply, Fault> {
        input.validate().map_err(|fault| Fault(fault.0))?;
        self.enqueue(id, Event::Composition(input))
    }
    fn enqueue(&mut self, id: u64, event: Event) -> Result<Reply, Fault> {
        let unit = self.runtime.host().unit(id)?;
        if !self.runtime.host().attached(unit) {
            return Err(Fault("unit has no web content".into()));
        }
        if self.queue.len() >= 256 {
            return Err(Fault("event queue is full".into()));
        }
        self.queue.push_back(Signal { unit, event });
        Ok(Reply::Ack)
    }
    pub(super) fn point(&mut self, id: u64, x: usize, y: usize) -> Result<Reply, Fault> {
        let unit = self.runtime.host().unit(id)?;
        if self.queue.len() >= 256 {
            return Err(Fault("event queue is full".into()));
        }
        let signal = self.runtime.host().stamp(Signal {
            unit,
            event: Event::Pointer { x, y },
        });
        self.queue.push_back(signal);
        Ok(Reply::Ack)
    }

    pub(super) fn click(&mut self, id: u64) -> Result<Reply, Fault> {
        let unit = self.runtime.host().unit(id)?;
        if self.runtime.host().attached(unit) {
            return Err(Fault("web content requires pointer coordinates".into()));
        }
        if self.queue.len() >= 256 {
            return Err(Fault("event queue is full".into()));
        }
        self.queue.push_back(Signal {
            unit,
            event: Event::Click,
        });
        Ok(Reply::Ack)
    }
}

impl<B: Backend, R: Renderer> Session<B, R> {
    pub(super) fn cache(
        &mut self,
        unit: u64,
        id: u64,
        command: shared::cache::Command,
    ) -> Result<Reply, Fault> {
        let unit = self.runtime.host().unit(unit)?;
        match self.runtime.host().cache(unit, id, command)? {
            crate::platform::Evaluation::Value(display) => Ok(Reply::Evaluation { display }),
            crate::platform::Evaluation::Queued(job) => Ok(Reply::Accepted { job, record: None }),
        }
    }
}
