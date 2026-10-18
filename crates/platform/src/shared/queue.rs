use super::{Event, Phase};
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Exhausted;

#[derive(Default)]
pub(crate) struct Queue {
    events: RefCell<VecDeque<Event>>,
    closed: Cell<bool>,
}

impl Queue {
    pub(crate) fn push(&self, event: Event) -> Result<(), Exhausted> {
        if self.closed.get() {
            return Ok(());
        }
        let mut events = self.events.borrow_mut();
        if matches!(
            (&event, events.back()),
            (
                Event::Pointer {
                    phase: Phase::Move,
                    ..
                },
                Some(Event::Pointer {
                    phase: Phase::Move,
                    ..
                })
            ) | (Event::Viewport { .. }, Some(Event::Viewport { .. }))
        ) {
            events.pop_back();
        }
        if events.len() == 512 {
            return Err(Exhausted);
        }
        events.push_back(event);
        Ok(())
    }

    pub(crate) fn drain(&self) -> Vec<Event> {
        let mut events = self.events.borrow_mut();
        let count = events.len().min(16);
        events.drain(..count).collect()
    }

    pub(crate) fn pending(&self) -> bool {
        !self.events.borrow().is_empty()
    }

    pub(crate) fn close(&self) {
        self.closed.set(true);
        self.events.borrow_mut().clear();
    }

    pub(crate) fn closed(&self) -> bool {
        self.closed.get()
    }
}

#[cfg(test)]
#[path = "../../tests/shared/queue.rs"]
mod checks;
