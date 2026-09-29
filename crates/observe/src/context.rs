use crate::{Guard, Kind, Session};
use std::cell::RefCell;

thread_local! {
    static STACK: RefCell<Vec<(u64, Option<Context>)>> = const { RefCell::new(Vec::new()) };
    static THREAD: u64 = crate::session::identity();
}
#[derive(Clone)]
pub struct Context {
    pub(crate) session: Session,
    pub(crate) parent: Option<u64>,
}
pub fn current() -> Option<Context> {
    STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .and_then(|(_, context)| context.clone())
    })
}
pub(crate) fn thread() -> u64 {
    THREAD.with(|id| *id)
}
pub(crate) fn push(id: u64, context: Option<Context>) {
    STACK.with(|stack| stack.borrow_mut().push((id, context)));
}
pub(crate) fn remove(id: u64) {
    STACK.with(|stack| stack.borrow_mut().retain(|context| context.0 != id));
}
impl Context {
    pub fn scope(&self, name: impl Into<String>) -> Guard {
        self.open(name.into(), Kind::Scope)
    }
    pub(crate) fn open(&self, name: String, kind: Kind) -> Guard {
        let mut state = self.session.0.state.lock().expect("observation state");
        if state.stopped || state.admitted >= self.session.0.options.limit {
            state.dropped = state.dropped.saturating_add(1);
            return Guard::blocked();
        }
        state.admitted += 1;
        state.open += 1;
        drop(state);
        Guard::new(self.clone(), name, kind)
    }
}
