use crate::{Context, Kind, Outcome, Record, context, native};
use std::{marker::PhantomData, rc::Rc};

pub struct Guard {
    active: Option<Active>,
    local: PhantomData<Rc<()>>,
    marker: Option<u64>,
}
struct Active {
    context: Context,
    record: Record,
    cpu: Option<u64>,
}
impl Guard {
    pub(crate) fn empty() -> Self {
        Self {
            active: None,
            marker: None,
            local: PhantomData,
        }
    }
    pub(crate) fn blocked() -> Self {
        let id = crate::session::identity();
        context::push(id, None);
        Self {
            marker: Some(id),
            active: None,
            local: PhantomData,
        }
    }
    pub(crate) fn new(context: Context, name: String, kind: Kind) -> Self {
        let id = crate::session::identity();
        let options = context.session.0.options;
        let signpost =
            options.native && native::begin(id, context.session.0.id, context.parent, &name);
        let record = Record {
            id,
            parent: context.parent,
            enclosing: crate::current()
                .filter(|outer| outer.session.0.id == context.session.0.id)
                .and_then(|outer| outer.parent),
            name,
            kind,
            thread: context::thread(),
            native: native::thread(),
            start: context.session.0.elapsed(),
            end: 0,
            cpu: None,
            outcome: Outcome::Abandoned,
            signpost,
        };
        let cpu = options.cpu.then(native::cpu).flatten();
        context::push(
            id,
            Some(Context {
                session: context.session.clone(),
                parent: Some(id),
            }),
        );
        Self {
            marker: Some(id),
            active: Some(Active {
                context,
                record,
                cpu,
            }),
            local: PhantomData,
        }
    }
    pub fn finish(&mut self, success: bool) {
        self.close(if success {
            Outcome::Success
        } else {
            Outcome::Failure
        });
    }
    fn close(&mut self, outcome: Outcome) {
        if let Some(id) = self.marker.take() {
            context::remove(id);
        }
        let Some(mut active) = self.active.take() else {
            return;
        };
        active.record.end = active.context.session.0.elapsed();
        active.record.cpu = active
            .cpu
            .and_then(|start| native::cpu()?.checked_sub(start));
        active.record.outcome = outcome;
        if active.record.signpost {
            native::end(active.record.id, outcome);
        }
        let mut state = active
            .context
            .session
            .0
            .state
            .lock()
            .expect("observation state");
        state.open -= 1;
        state.records.push(active.record);
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.close(if std::thread::panicking() {
            Outcome::Panicked
        } else {
            Outcome::Abandoned
        });
    }
}
