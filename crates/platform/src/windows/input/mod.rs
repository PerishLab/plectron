mod key;
mod pointer;
mod text;
use super::Error;
use crate::shared::{Event, Phase};
use std::cell::{Cell, RefCell};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, WPARAM},
    UI::{Controls::WM_MOUSELEAVE, WindowsAndMessaging::*},
};

#[derive(Default)]
pub(super) struct Queue {
    events: crate::shared::Queue,
    text: RefCell<text::Decoder>,
    captured: Cell<bool>,
    tracking: Cell<bool>,
    outside: Cell<bool>,
    left: Cell<bool>,
    focused: Cell<bool>,
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
    pub fn drain(&self) -> Vec<Event> {
        self.events.drain()
    }
    pub fn pending(&self) -> bool {
        self.events.pending()
    }
    pub fn close(&self) {
        self.events.close();
        self.text.borrow_mut().clear();
    }
    pub fn dispatch(
        &self,
        handle: HWND,
        message: u32,
        data: (WPARAM, LPARAM),
    ) -> Result<bool, Error> {
        if self.events.closed() {
            return Ok(false);
        }
        match message {
            WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_MOUSEWHEEL => {
                self.pointer(handle, message, data)
            }
            WM_MOUSELEAVE => self.leave(handle),
            WM_CAPTURECHANGED | WM_CANCELMODE => self.cancel(handle, message == WM_CANCELMODE),
            WM_SETFOCUS => {
                self.focused.set(true);
                Ok(true)
            }
            WM_KILLFOCUS => {
                self.focused.set(false);
                self.text.borrow_mut().clear();
                self.cancel(handle, true)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN if self.focused.get() => {
                if let Some(input) = key::stroke(data) {
                    self.push(Event::Input(input))?;
                }
                Ok(message == WM_KEYDOWN)
            }
            WM_CHAR if self.focused.get() => {
                if let Some(text) = self.text.borrow_mut().unit(data.0)? {
                    self.push(Event::Input(crate::shared::Keyboard::Text(text)))?;
                }
                Ok(true)
            }
            WM_IME_STARTCOMPOSITION | WM_IME_COMPOSITION | WM_IME_ENDCOMPOSITION | WM_IME_CHAR => {
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn cancel(&self, handle: HWND, release: bool) -> Result<bool, Error> {
        if self.captured.replace(false) {
            self.push(Event::Pointer {
                position: None,
                phase: Phase::Cancel,
            })?;
            if release {
                pointer::release(handle);
            }
        }
        self.exit()?;
        Ok(true)
    }
    fn exit(&self) -> Result<(), Error> {
        if self.captured.get() {
            return Ok(());
        }
        self.tracking.set(false);
        if !self.outside.get() || self.left.replace(true) {
            return Ok(());
        }
        self.push(Event::Pointer {
            position: None,
            phase: Phase::Move,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound() {
        let queue = Queue::default();
        for _ in 0..512 {
            queue.push(Event::Scroll { delta: 1 }).unwrap();
        }
        assert!(queue.push(Event::Scroll { delta: 1 }).is_err());
        assert_eq!(queue.drain().len(), 16);
        assert!(queue.pending());
        queue.close();
        queue.push(Event::Scroll { delta: 1 }).unwrap();
        assert!(!queue.pending());
    }
}
