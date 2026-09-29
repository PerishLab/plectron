use crate::contract::{Event, Fault};
use crate::resource::{Loader, Services};
use std::ops::Deref;
use webengine::{Action, Display, Hit, Page, Snapshot, State, Worker};

pub enum Evaluation {
    Value(String),
    Queued(u64),
}
pub(super) enum Instance {
    Local(Box<Page>),
    Thread(Box<Worker>),
}
impl Instance {
    pub fn pacing(&self) -> Result<graphics::shared::Pacing, Fault> {
        match self {
            Self::Local(page) => Ok(page.pacing()),
            Self::Thread(worker) => worker.pacing(),
        }
    }

    pub fn pulse(&mut self, pulse: graphics::shared::Pulse) -> Result<bool, Fault> {
        match self {
            Self::Local(page) => page.pulse(pulse),
            Self::Thread(worker) => worker.pulse(pulse),
        }
    }

    pub fn new(
        url: &str,
        extent: domengine::Extent,
        loader: Option<Loader>,
        options: (config::Store, Option<shared::Notify>),
    ) -> Result<Self, Fault> {
        match loader {
            Some(loader) => Ok(Self::Thread(Box::new(Worker::notified(
                url, extent, loader, options,
            )?))),
            None => Ok(Self::Local(Box::new(Page::configured(
                url, extent, options.0,
            )?))),
        }
    }
    pub fn job(&self, id: u64) -> Option<&webengine::Job> {
        match self {
            Self::Thread(worker) => worker.job(id),
            Self::Local(_) => None,
        }
    }
    pub fn inspect(&mut self) -> Snapshot {
        match self {
            Self::Local(page) => page.inspect(),
            Self::Thread(worker) => worker.inspect(),
        }
    }
    pub fn cache(&mut self, id: u64, command: shared::cache::Command) -> Result<Evaluation, Fault> {
        match self {
            Self::Local(page) => page.cache(id, command).map(Evaluation::Value),
            Self::Thread(worker) => worker
                .submit(Action::Cache { id, command })
                .map(Evaluation::Queued),
        }
    }
    pub fn evaluate(&mut self, source: &str) -> Result<Evaluation, Fault> {
        match self {
            Self::Local(page) => page.evaluate(source).map(Evaluation::Value),
            Self::Thread(worker) => worker
                .submit(Action::Evaluate(source.into()))
                .map(Evaluation::Queued),
        }
    }
    pub fn collect(&mut self) -> Result<Evaluation, Fault> {
        match self {
            Self::Local(page) => page
                .collect()
                .map(|collection| Evaluation::Value(collection.to_string())),
            Self::Thread(worker) => worker.submit(Action::Collect).map(Evaluation::Queued),
        }
    }
    pub fn dispatch(&mut self, hit: &Hit) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.dispatch(hit),
            Self::Thread(worker) => worker.submit(Action::Dispatch(hit.clone())).map(|_| ()),
        }
    }
    pub fn transition(&mut self, from: Option<&Hit>, to: Option<&Hit>) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.transition(from, to),
            Self::Thread(worker) => worker
                .submit(Action::Transition {
                    from: from.cloned(),
                    to: to.cloned(),
                })
                .map(|_| ()),
        }
    }
    pub fn input(&mut self, input: &webengine::keyboard::Input) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.input(input).map(|_| ()),
            Self::Thread(worker) => worker.submit(Action::Input(input.clone())).map(|_| ()),
        }
    }
    pub fn edit(&mut self, edit: &webengine::Edit) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.edit(edit),
            Self::Thread(worker) => worker.submit(Action::Edit(edit.clone())).map(|_| ()),
        }
    }
    pub fn compose(&mut self, input: &webengine::ime::Input) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.composition(input).map(|_| ()),
            Self::Thread(worker) => worker
                .submit(Action::Composition(input.clone()))
                .map(|_| ()),
        }
    }
    pub fn receive(&mut self, text: String) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.receive(text),
            Self::Thread(worker) => worker.submit(Action::Receive(text)).map(|_| ()),
        }
    }
    pub fn advance(&mut self, services: &mut Services) -> Result<Vec<Event>, Fault> {
        if let Self::Local(page) = self {
            if page.state() == State::Loading {
                page.load(services)?;
            }
            if page.state() == State::Failed {
                return Ok(Vec::new());
            }
            page.render()?;
            return Ok(page.messages().into_iter().map(Event::Message).collect());
        }
        self.poll()
    }
    pub fn poll(&mut self) -> Result<Vec<Event>, Fault> {
        if let Self::Local(page) = self {
            if page.state() == State::Ready {
                page.poll()?;
            }
            return Ok(page.messages().into_iter().map(Event::Message).collect());
        }
        let Self::Thread(worker) = self else {
            unreachable!()
        };
        worker.poll()?;
        let mut events: Vec<_> = worker.messages().into_iter().map(Event::Message).collect();
        events.extend(worker.errors().into_iter().map(Event::Error));
        Ok(events)
    }
    pub fn hold(&mut self) -> Result<u64, Fault> {
        let Self::Thread(worker) = self else {
            return Err(Fault("hold requires threaded content".into()));
        };
        worker.hold()
    }
    pub fn unhold(&mut self) -> Result<(), Fault> {
        let Self::Thread(worker) = self else {
            return Err(Fault("unhold requires threaded content".into()));
        };
        worker.unhold()
    }
    pub fn close(&mut self) {
        match self {
            Self::Local(page) => page.close(),
            Self::Thread(worker) => worker.close(),
        }
    }
}
impl Deref for Instance {
    type Target = Display;
    fn deref(&self) -> &Display {
        match self {
            Self::Local(page) => page,
            Self::Thread(worker) => worker,
        }
    }
}

impl Instance {
    pub fn frame(&mut self) -> Option<graphics::shared::Frame> {
        match self {
            Self::Local(page) => page.frame(),
            Self::Thread(worker) => worker.frame(),
        }
    }
    pub fn complete(&mut self, frame: u64, result: Result<(), Fault>) -> Result<bool, Fault> {
        match self {
            Self::Local(page) => page.complete(frame, result),
            Self::Thread(worker) => worker.complete(frame, result),
        }
    }
    pub fn scroll(&mut self, delta: i64) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.scroll(delta),
            Self::Thread(worker) => worker.scroll(delta),
        }
    }
    pub fn pan(&mut self, scene: u64, space: u64, delta: i64) -> Result<(), Fault> {
        match self {
            Self::Local(page) => page.pan(scene, space, delta),
            Self::Thread(worker) => worker.pan(scene, space, delta),
        }
    }
    pub fn wheel(
        &mut self,
        x: usize,
        y: usize,
        delta: i64,
    ) -> Result<webengine::Disposition, Fault> {
        match self {
            Self::Local(page) => page.wheel(x, y, delta),
            Self::Thread(worker) => worker.wheel(x, y, delta),
        }
    }
}
