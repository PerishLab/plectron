use super::{Error, window::Window};
use crate::shared::{Composition as Input, Event, Focus, Id, Keyboard, Selection, Signal, Unit};
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::NonNull;

#[repr(C)]
struct Native {
    kind: u32,
    id: u64,
    text: *mut c_char,
    start: usize,
    end: usize,
}

pub(super) struct View(NonNull<c_void>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Text {
    Mark {
        id: u64,
        text: String,
        start: usize,
        end: usize,
    },
    Commit {
        id: u64,
        text: String,
    },
    Cancel {
        id: u64,
    },
    Insert(String),
}

pub(super) fn trace() -> bool {
    unsafe extern "C" {
        fn plectron_ime_trace_enabled() -> c_int;
    }
    // SAFETY: The native query reads a process-local diagnostic flag.
    unsafe { plectron_ime_trace_enabled() != 0 }
}

impl View {
    pub fn new(window: &Window) -> Result<Self, Error> {
        unsafe extern "C" {
            fn plectron_ime_attach(parent: *mut c_void) -> *mut c_void;
        }
        // SAFETY: The native window retains its host NSView during attachment.
        let view =
            unsafe { plectron_ime_attach((window.view() as *const _ as *mut c_void).cast()) };
        NonNull::new(view)
            .map(Self)
            .ok_or_else(|| Error("text input view could not be attached".into()))
    }

    pub fn cancel(&mut self) {
        unsafe extern "C" {
            fn plectron_ime_cancel(view: *mut c_void);
        }
        // SAFETY: The owned view remains attached until Drop, on the desktop thread.
        unsafe { plectron_ime_cancel(self.0.as_ptr()) };
    }

    pub fn active(&self) -> bool {
        unsafe extern "C" {
            fn plectron_ime_active(view: *mut c_void) -> c_int;
        }
        // SAFETY: The attached view and its window are queried on the desktop thread.
        unsafe { plectron_ime_active(self.0.as_ptr()) != 0 }
    }

    pub fn context(&mut self, context: Option<&Focus>) -> Result<(), Error> {
        unsafe extern "C" {
            fn plectron_ime_context(
                view: *mut c_void,
                target: usize,
                focused: c_int,
                text: *const c_char,
                start: usize,
                end: usize,
                x: f64,
                y: f64,
                height: f64,
            );
        }
        let text = context
            .map(|context| CString::new(context.text.as_str()))
            .transpose()
            .map_err(|error| Error(error.to_string()))?;
        let selection = context.map(|context| context.selection);
        let start = selection.map_or(0, |selection| selection.anchor);
        let end = selection.map_or(0, |selection| selection.focus);
        let start = context.map_or(0, |context| utf16(&context.text, start));
        let end = context.map_or(0, |context| utf16(&context.text, end));
        let caret = context.map(|context| context.caret);
        // SAFETY: The view and optional C string are valid for this synchronous desktop-thread call.
        unsafe {
            plectron_ime_context(
                self.0.as_ptr(),
                context.map_or(0, |context| context.target),
                i32::from(context.is_some()),
                text.as_ref().map_or(std::ptr::null(), |text| text.as_ptr()),
                start.min(end),
                start.max(end),
                caret.map_or(0.0, |rect| rect.x as f64),
                caret.map_or(0.0, |rect| rect.y as f64),
                caret.map_or(18.0, |rect| rect.height as f64),
            );
        }
        Ok(())
    }

    pub fn drain(&mut self, unit: Unit, signals: &mut Vec<Signal>) -> Result<(), Error> {
        unsafe extern "C" {
            fn plectron_ime_pop(view: *mut c_void, event: *mut Native) -> c_int;
            fn plectron_ime_free(text: *mut c_char);
        }
        loop {
            let mut native = Native {
                kind: 0,
                id: 0,
                text: std::ptr::null_mut(),
                start: 0,
                end: 0,
            };
            // SAFETY: The view and writable event record are valid on the desktop thread.
            let ready = unsafe { plectron_ime_pop(self.0.as_ptr(), &mut native) };
            if ready == 0 {
                return Ok(());
            }
            // SAFETY: A successful pop returns a non-null terminated string until free.
            let text = unsafe { CStr::from_ptr(native.text) }
                .to_str()
                .map(str::to_string)
                .map_err(|error| Error(error.to_string()));
            let input = text.and_then(|text| match native.kind {
                1 => Text::Mark {
                    id: native.id,
                    text,
                    start: native.start,
                    end: native.end,
                }
                .event(),
                2 => Text::Commit {
                    id: native.id,
                    text,
                }
                .event(),
                3 => Text::Cancel { id: native.id }.event(),
                4 => Text::Insert(text).event(),
                _ => Err(Error("unknown native text input event".into())),
            });
            // SAFETY: pop transfers ownership of the allocated string to this caller.
            unsafe { plectron_ime_free(native.text) };
            signals.push(Signal {
                unit,
                event: input?,
            });
        }
    }
}

impl Drop for View {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn plectron_ime_detach(view: *mut c_void);
        }
        // SAFETY: The owned view is detached once, before its parent window drops.
        unsafe { plectron_ime_detach(self.0.as_ptr()) };
    }
}

fn scalar(text: &str, position: usize) -> Result<usize, Error> {
    let mut utf16 = 0;
    for (index, character) in text.chars().enumerate() {
        if utf16 == position {
            return Ok(index);
        }
        utf16 += character.len_utf16();
    }
    if utf16 == position {
        return Ok(text.chars().count());
    }
    Err(Error("native text selection splits a scalar".into()))
}

fn utf16(text: &str, position: usize) -> usize {
    text.chars().take(position).map(char::len_utf16).sum()
}

impl Text {
    pub fn event(self) -> Result<Event, Error> {
        match self {
            Self::Mark {
                id,
                text,
                start,
                end,
            } => Ok(Event::Composition(Input::Mark {
                id: Id(id),
                selection: Selection {
                    anchor: scalar(&text, start)?,
                    focus: scalar(&text, end)?,
                },
                text,
            })),
            Self::Commit { id, text } => Ok(Event::Composition(Input::Commit { id: Id(id), text })),
            Self::Cancel { id } => Ok(Event::Composition(Input::Cancel { id: Id(id) })),
            Self::Insert(text) => Ok(Event::Input(Keyboard::Text(text))),
        }
    }
}
