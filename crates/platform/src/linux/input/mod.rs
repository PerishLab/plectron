mod compose;
mod key;
mod pointer;
mod text;
use super::Error;
use crate::shared::{Event, Phase};
use std::cell::{Cell, RefCell};
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{NotifyDetail, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Effect {
    Idle,
    Focus(Timestamp),
}

#[derive(Default)]
pub(super) struct Queue {
    events: crate::shared::Queue,
    keys: RefCell<key::Keys>,
    captured: Cell<bool>,
    outside: Cell<bool>,
    left: Cell<bool>,
    focused: Cell<bool>,
    target: Cell<Option<usize>>,
    density: Cell<Option<u32>>,
}
impl Queue {
    pub(super) fn push(&self, event: Event) -> Result<(), Error> {
        self.events
            .push(event)
            .map_err(|_| Error("native input queue exhausted".into()))
    }
    pub(super) fn density(&self, dpi: u32) {
        self.density.set(Some(dpi));
    }
    pub(super) fn remap(&self, first: u8, width: u8, keysyms: Vec<u32>) -> Result<(), Error> {
        let cancel = self
            .keys
            .borrow_mut()
            .remap(key::Keymap::new(first, width, keysyms));
        self.end(cancel)
    }
    pub(super) fn target(&self, target: Option<usize>) -> Result<(), Error> {
        if self.target.replace(target) == target {
            return Ok(());
        }
        let cancel = self.keys.borrow_mut().refocus();
        self.end(cancel)
    }
    fn end(&self, cancel: Option<Event>) -> Result<(), Error> {
        cancel.map_or(Ok(()), |event| self.push(event))
    }
    pub fn drain(&self) -> Vec<Event> {
        self.events.drain()
    }
    pub fn pending(&self) -> bool {
        self.events.pending()
    }
    pub fn close(&self) {
        self.events.close();
        self.keys.borrow_mut().clear();
    }
    pub fn dispatch(&self, event: &XEvent) -> Result<Effect, Error> {
        if self.events.closed() {
            return Ok(Effect::Idle);
        }
        match event {
            XEvent::MotionNotify(motion) => self.motion(motion),
            XEvent::ButtonPress(press) => self.press(press),
            XEvent::ButtonRelease(release) => self.release(release),
            XEvent::EnterNotify(enter) if enter.detail != NotifyDetail::INFERIOR => {
                self.enter(enter)
            }
            XEvent::LeaveNotify(leave) if leave.detail != NotifyDetail::INFERIOR => {
                self.leave(leave.mode)
            }
            XEvent::FocusIn(focus) if owned(focus.detail) => {
                self.focused.set(true);
                Ok(Effect::Idle)
            }
            XEvent::FocusOut(focus) if owned(focus.detail) => {
                self.focused.set(false);
                let cancel = self.keys.borrow_mut().clear();
                self.end(cancel)?;
                self.cancel()
            }
            XEvent::KeyPress(press) if self.focused.get() => {
                let events =
                    self.keys
                        .borrow_mut()
                        .press(press.detail, u16::from(press.state), press.time);
                for event in events {
                    self.push(event)?;
                }
                Ok(Effect::Idle)
            }
            XEvent::KeyRelease(release) => {
                self.keys.borrow_mut().release(release.detail, release.time);
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
        self.exit()
    }
}

fn owned(detail: NotifyDetail) -> bool {
    detail != NotifyDetail::INFERIOR && detail != NotifyDetail::POINTER
}

#[cfg(test)]
#[path = "../../../tests/linux/compose.rs"]
mod native;
#[cfg(test)]
mod route;
