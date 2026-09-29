use super::protocol::{Action, Report, Task, Turn};
use super::publication::Publisher;
use crate::{Fault, page::Content, resource::Provider};
use domengine::Extent;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::Receiver,
};

pub(super) fn run<P: Provider>(
    source: (String, Extent, P),
    channels: (u64, Receiver<Task>, Publisher, Option<observe::Context>),
    closed: Arc<AtomicBool>,
    setup: (
        config::Store,
        std::time::Instant,
        crate::page::AnimationController,
    ),
) {
    let (url, extent, mut provider) = source;
    let (id, inbox, outbox, context) = channels;
    let mut scope = observe::resume(context.as_ref(), "content.load");
    let (config, clock, animation) = setup;
    let Ok(mut content) = Content::threaded(&url, extent, config, (clock, animation)) else {
        return;
    };
    if outbox.send(Report::Started(id)).is_err() {
        return;
    }
    let result = content
        .load(&mut provider)
        .and_then(|()| content.commit())
        .map(|()| String::new());
    let success = result.is_ok();
    if closed.load(Ordering::Acquire) || !content.finished(id, result, &outbox) {
        return;
    }
    scope.finish(success);
    loop {
        if closed.load(Ordering::Acquire) {
            break;
        }
        let Some(task) = next(&inbox, (&mut content, &outbox)) else {
            break;
        };
        let mut scope = task.context.enter("content.execute");
        if closed.load(Ordering::Acquire) || outbox.send(Report::Started(task.id)).is_err() {
            break;
        }
        if matches!(task.turn, Turn::Hold(_)) && outbox.send(Report::Held(task.id)).is_err() {
            break;
        }
        if let Some(runtime) = &mut content.runtime {
            runtime.addons().events.job = Some(task.id);
        }
        let result = content.perform(task.turn);
        let success = result.is_ok();
        if closed.load(Ordering::Acquire) || !content.finished(task.id, result, &outbox) {
            break;
        }
        if !content.pump(&outbox) {
            break;
        }
        scope.finish(success);
    }
    content.close();
}
impl Content {
    fn perform(&mut self, turn: Turn) -> Result<String, Fault> {
        let action = match turn {
            Turn::Execute(action) => action,
            Turn::Scroll(changes, visual) => {
                self.scrolling(changes)?;
                self.viewport(visual)?;
                self.render()?;
                self.commit()?;
                return Ok(String::new());
            }
            Turn::Hold(ticket) => {
                return ticket.wait().map(|()| String::new()).map_err(crate::error);
            }
        };
        let result = match action {
            Action::Demand(visual) => {
                self.viewport(visual)?;
                self.commit()?;
                return Ok(String::new());
            }
            Action::Geometry(id) => return self.geometry(&id).map(|rect| format!("{rect:?}")),
            Action::Cache { id, command } => return self.cache(id, command),
            Action::Evaluate(source) => self.evaluate(&source),
            Action::Collect => self.collect([]).map(|collection| collection.to_string()),
            Action::Dispatch(hit) => self.target(hit.node()).map(|()| String::new()),
            Action::Transition { from, to } => self
                .transition(
                    from.as_ref().and_then(crate::Hit::node),
                    to.as_ref().and_then(crate::Hit::node),
                )
                .map(|()| String::new()),
            Action::Input(input) => self.input(&input).map(|outcome| {
                format!("handled={} consumed={}", outcome.handled, outcome.consumed)
            }),
            Action::Composition(input) => self.compose(&input).map(|handled| handled.to_string()),
            Action::Edit(edit) => self.edit(&edit).map(|()| String::new()),
            Action::Receive(text) => self.receive(text).map(|()| String::new()),
        };
        let value = result?;
        self.render()?;
        self.commit()?;
        Ok(value)
    }

    fn commit(&mut self) -> Result<(), Fault> {
        for _ in 0..=32 {
            if !self.finalize()? {
                return Ok(());
            }
        }
        Err(Fault("threaded scene did not converge".into()))
    }
    fn finished(&mut self, id: u64, result: Result<String, Fault>, outbox: &Publisher) -> bool {
        observe::call("content.publish", || {
            outbox.send(Report::Finished {
                id,
                publication: Box::new(self.publish()),
                messages: self.messages(),
                result: result.map_err(|error| error.to_string()),
                produced: std::time::Instant::now(),
            })
        })
        .is_ok()
    }

    fn updated(&mut self, result: Result<(), Fault>, outbox: &Publisher) -> bool {
        observe::call("content.publish", || {
            outbox.send(Report::Update {
                publication: Box::new(self.publish()),
                messages: self.messages(),
                result: result.map_err(|fault| fault.to_string()),
            })
        })
        .is_ok()
    }

    fn pump(&mut self, outbox: &Publisher) -> bool {
        let _detached = observe::resume(None, "content.poll");
        if !self.pending() {
            return true;
        }
        if let Some(runtime) = &mut self.runtime {
            runtime.addons().events.job = None;
        }
        let result = self.poll();
        matches!(result, Ok(false)) || self.updated(result.map(|_| ()), outbox)
    }
}
fn next(inbox: &Receiver<Task>, context: (&mut Content, &Publisher)) -> Option<Task> {
    let (content, outbox) = context;
    loop {
        match receive(inbox, content) {
            Ok(task) => return Some(task),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) if !tick(content, outbox) => {
                return None;
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
            Err(_) => return None,
        }
    }
}

fn receive(
    inbox: &Receiver<Task>,
    content: &mut Content,
) -> Result<Task, std::sync::mpsc::RecvTimeoutError> {
    let pending = content
        .pending()
        .then_some(std::time::Duration::from_millis(4));
    match pending {
        Some(wait) => inbox.recv_timeout(wait),
        None => inbox
            .recv()
            .map_err(|_| std::sync::mpsc::RecvTimeoutError::Disconnected),
    }
}

fn tick(content: &mut Content, outbox: &Publisher) -> bool {
    content.pump(outbox)
}
