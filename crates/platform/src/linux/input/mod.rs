mod key;
mod pointer;
mod text;
use super::Error;
use crate::shared::{Event, Phase};
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{NotifyDetail, NotifyMode, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    Idle,
    Focus(Timestamp),
}

#[derive(Default)]
pub(super) struct Queue {
    events: RefCell<VecDeque<Event>>,
    keys: RefCell<key::Keys>,
    captured: Cell<bool>,
    focused: Cell<bool>,
    closed: Cell<bool>,
    density: Cell<Option<u32>>,
}
impl Queue {
    pub(super) fn push(&self, event: Event) -> Result<(), Error> {
        if self.closed.get() {
            return Ok(());
        }
        let mut events = self.events.borrow_mut();
        if matches!(
            &event,
            Event::Pointer {
                phase: Phase::Move,
                ..
            }
        ) && matches!(
            events.back(),
            Some(Event::Pointer {
                phase: Phase::Move,
                ..
            })
        ) {
            events.pop_back();
        }
        if matches!(&event, Event::Viewport { .. })
            && matches!(events.back(), Some(Event::Viewport { .. }))
        {
            events.pop_back();
        }
        if events.len() == 512 {
            return Err(Error("native input queue exhausted".into()));
        }
        events.push_back(event);
        Ok(())
    }
    pub(super) fn density(&self, dpi: u32) {
        self.density.set(Some(dpi));
    }
    pub(super) fn remap(&self, first: u8, width: u8, keysyms: Vec<u32>) {
        self.keys
            .borrow_mut()
            .remap(key::Keymap::new(first, width, keysyms));
    }
    pub fn drain(&self) -> Vec<Event> {
        let mut events = self.events.borrow_mut();
        let count = events.len().min(16);
        events.drain(..count).collect()
    }
    pub fn pending(&self) -> bool {
        !self.events.borrow().is_empty()
    }
    pub fn close(&self) {
        self.closed.set(true);
        self.events.borrow_mut().clear();
        self.keys.borrow_mut().clear();
    }
    pub fn dispatch(&self, event: &XEvent, extent: (u16, u16)) -> Result<Effect, Error> {
        if self.closed.get() {
            return Ok(Effect::Idle);
        }
        match event {
            XEvent::MotionNotify(motion) => self.motion(motion),
            XEvent::ButtonPress(press) => self.press(press),
            XEvent::ButtonRelease(release) => self.release(release, extent),
            XEvent::LeaveNotify(leave) if leave.mode == NotifyMode::GRAB => self.cancel(),
            XEvent::FocusIn(focus) if owned(focus.detail) => {
                self.focused.set(true);
                Ok(Effect::Idle)
            }
            XEvent::FocusOut(focus) if owned(focus.detail) => {
                self.focused.set(false);
                self.keys.borrow_mut().clear();
                self.cancel()
            }
            XEvent::KeyPress(press) if self.focused.get() => {
                let inputs = self
                    .keys
                    .borrow_mut()
                    .press(press.detail, u16::from(press.state));
                for input in inputs {
                    self.push(Event::Input(input))?;
                }
                Ok(Effect::Idle)
            }
            XEvent::KeyRelease(release) => {
                self.keys.borrow_mut().release(release.detail);
                Ok(Effect::Idle)
            }
            _ => Ok(Effect::Idle),
        }
    }
    fn cancel(&self) -> Result<Effect, Error> {
        if self.captured.replace(false) {
            self.push(Event::Pointer {
                position: None,
                phase: Phase::Cancel,
            })?;
        }
        Ok(Effect::Idle)
    }
}

fn owned(detail: NotifyDetail) -> bool {
    detail != NotifyDetail::INFERIOR && detail != NotifyDetail::POINTER
}

#[cfg(test)]
mod route;
