mod clipboard;
mod ime;
mod imm;
mod key;
mod pointer;
#[cfg(test)]
mod tests;
mod text;
use super::{Desktop, Error};
use crate::shared::{Clipboard, Event, Focus, Input, Phase, Signal, Unit};
use std::{
    cell::{Cell, RefCell},
    sync::Arc,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::{Controls::WM_MOUSELEAVE, WindowsAndMessaging::*},
};

pub(super) fn context(handle: HWND, data: (WPARAM, LPARAM)) -> LRESULT {
    imm::context(handle, data)
}

#[derive(Default)]
pub(super) struct Queue {
    events: crate::shared::Queue,
    text: RefCell<text::Decoder>,
    ime: RefCell<ime::State>,
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
        self.ime.borrow_mut().clear();
    }
    pub fn focus(&self, handle: HWND, dpi: u32, context: Option<&Focus>) -> Result<(), Error> {
        if self.events.closed() {
            return Ok(());
        }
        let mut events = Vec::new();
        let (cancelled, caret) = self.ime.borrow_mut().focus(context, dpi, &mut events);
        if cancelled {
            imm::abort(handle);
        }
        self.emit(events)?;
        if let Some(caret) = caret {
            imm::place(handle, caret, dpi);
        }
        Ok(())
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
                self.abort(handle)?;
                self.cancel(handle, true)
            }
            WM_KEYDOWN | WM_SYSKEYDOWN if self.focused.get() => {
                if self.ime.borrow().active() {
                    return Ok(message == WM_KEYDOWN);
                }
                if data.0 != 0xe5 {
                    self.ime.borrow_mut().clear();
                }
                if let Some(input) = key::stroke(data) {
                    self.push(Event::Input(input))?;
                }
                Ok(message == WM_KEYDOWN)
            }
            WM_CHAR if self.focused.get() => {
                if self.ime.borrow_mut().suppress(data.0) {
                    self.text.borrow_mut().clear();
                    return Ok(true);
                }
                if let Some(text) = self.text.borrow_mut().unit(data.0)? {
                    self.push(Event::Input(crate::shared::Keyboard::Text(text)))?;
                }
                Ok(true)
            }
            WM_IME_STARTCOMPOSITION => {
                self.text.borrow_mut().clear();
                self.ime.borrow_mut().start();
                Ok(true)
            }
            WM_IME_COMPOSITION => {
                let mut events = Vec::new();
                self.ime
                    .borrow_mut()
                    .read(handle, data.1 as u32, &mut events)?;
                self.emit(events)?;
                Ok(true)
            }
            WM_IME_ENDCOMPOSITION => {
                let mut events = Vec::new();
                self.ime.borrow_mut().end(&mut events);
                self.emit(events)?;
                Ok(true)
            }
            WM_IME_CHAR => Ok(true),
            _ => Ok(false),
        }
    }
    fn emit(&self, events: Vec<Event>) -> Result<(), Error> {
        for event in events {
            self.push(event)?;
        }
        Ok(())
    }
    fn abort(&self, handle: HWND) -> Result<(), Error> {
        let mut events = Vec::new();
        let active = self.ime.borrow_mut().cancel(&mut events);
        if active {
            imm::abort(handle);
        }
        self.emit(events)
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

impl<E> Input<E> for Desktop<E> {
    fn poll(&mut self) -> Result<Vec<Signal>, E> {
        self.class
            .check()
            .and_then(|()| self.events.poll())
            .map_err(self.convert)?;
        self.settle();
        let units: Vec<Unit> = self.windows.keys().copied().collect();
        for unit in units {
            let window = self.windows.get(&unit).expect("checked window");
            window.result().map_err(self.convert)?;
            if window.accelerated() {
                if window.changed() {
                    self.graphics.refresh(unit, window).map_err(self.convert)?;
                }
            } else if let Ok(compute) = self.graphics.compute() {
                window.refresh(compute).map_err(self.convert)?;
            }
        }
        let mut signals = Vec::new();
        for (unit, window) in &self.windows {
            if window.closing() {
                signals.push(Signal {
                    unit: *unit,
                    event: Event::Close,
                });
            } else {
                signals.extend(
                    window
                        .input()
                        .into_iter()
                        .map(|event| Signal { unit: *unit, event }),
                );
                if window.pending() {
                    self.events.notifier().notify();
                }
            }
        }
        Ok(signals)
    }

    fn focus(&mut self, unit: Unit, context: Option<&Focus>) -> Result<(), E> {
        self.window(unit)
            .and_then(|window| window.focus(context))
            .map_err(self.convert)
    }

    fn clipboard(&self) -> Option<Arc<dyn Clipboard>> {
        Some(Arc::new(clipboard::System))
    }
}
