use super::Error;
use objc2_app_kit::NSView;
use std::ffi::{CString, c_char, c_void};
use std::marker::PhantomData;
use std::ptr::NonNull;
use std::rc::Rc;

#[repr(C)]
struct Record {
    kind: u32,
    code: u16,
    repeat: u8,
    flags: u64,
    x: f64,
    y: f64,
    delta: f64,
}

pub(super) enum Event {
    Close,
    Move { x: f64, y: f64 },
    Down { x: f64, y: f64 },
    Up { x: f64, y: f64 },
    Wheel { x: f64, y: f64, delta: f64 },
    Key { code: u16, repeat: bool, flags: u64 },
}

pub(super) struct Window {
    native: NonNull<c_void>,
    view: NonNull<NSView>,
    _main: PhantomData<Rc<()>>,
}

impl Window {
    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, Error> {
        unsafe extern "C" {
            fn plectron_window_create(
                title: *const c_char,
                width: usize,
                height: usize,
            ) -> *mut c_void;
            fn plectron_window_view(window: *mut c_void) -> *mut c_void;
        }
        let title = CString::new(title).map_err(|error| Error(error.to_string()))?;
        // SAFETY: Creation runs on the platform main thread with a valid C string.
        let native = NonNull::new(unsafe { plectron_window_create(title.as_ptr(), width, height) })
            .ok_or_else(|| Error("native window creation failed".into()))?;
        // SAFETY: A successfully created native window owns a non-null NSView.
        let view = NonNull::new(unsafe { plectron_window_view(native.as_ptr()) })
            .map(NonNull::cast)
            .ok_or_else(|| Error("native window view unavailable".into()))?;
        Ok(Self {
            native,
            view,
            _main: PhantomData,
        })
    }

    pub fn show(&self) {
        unsafe extern "C" {
            fn plectron_window_show(window: *mut c_void);
        }
        // SAFETY: The owned window is used on its platform thread.
        unsafe { plectron_window_show(self.native.as_ptr()) };
    }

    pub fn key(&self) {
        unsafe extern "C" {
            fn plectron_window_key(window: *mut c_void);
        }
        // SAFETY: The owned window is used on its platform thread.
        unsafe { plectron_window_key(self.native.as_ptr()) };
    }

    pub fn title(&self, title: &str) -> Result<(), Error> {
        unsafe extern "C" {
            fn plectron_window_title(window: *mut c_void, title: *const c_char);
        }
        let title = CString::new(title).map_err(|error| Error(error.to_string()))?;
        // SAFETY: The owned window and C string are valid for this synchronous call.
        unsafe { plectron_window_title(self.native.as_ptr(), title.as_ptr()) };
        Ok(())
    }

    pub fn view(&self) -> &NSView {
        // SAFETY: The native window retains its view for this wrapper's lifetime.
        unsafe { self.view.as_ref() }
    }

    pub fn scale(&self) -> f64 {
        unsafe extern "C" {
            fn plectron_window_scale(window: *mut c_void) -> f64;
        }
        // SAFETY: The owned window is queried on its platform thread.
        unsafe { plectron_window_scale(self.native.as_ptr()) }
    }

    pub fn present(&self, pixels: &[u32], width: usize, height: usize) -> Result<(), Error> {
        unsafe extern "C" {
            fn plectron_window_present(
                window: *mut c_void,
                pixels: *const u32,
                width: usize,
                height: usize,
            ) -> i32;
        }
        let size = width
            .checked_mul(height)
            .ok_or_else(|| Error("native image dimensions overflow".into()))?;
        if pixels.len() != size {
            return Err(Error("native image length mismatch".into()));
        }
        // SAFETY: The pixel slice remains valid for the synchronous copying call.
        if unsafe { plectron_window_present(self.native.as_ptr(), pixels.as_ptr(), width, height) }
            == 0
        {
            return Err(Error("native image presentation failed".into()));
        }
        Ok(())
    }

    pub fn drain(&self) -> Result<Vec<Event>, Error> {
        unsafe extern "C" {
            fn plectron_window_pop(window: *mut c_void, event: *mut Record) -> i32;
        }
        let mut events = Vec::new();
        loop {
            let mut record = Record {
                kind: 0,
                code: 0,
                repeat: 0,
                flags: 0,
                x: 0.0,
                y: 0.0,
                delta: 0.0,
            };
            // SAFETY: The record is writable and the owned window is on its platform thread.
            if unsafe { plectron_window_pop(self.native.as_ptr(), &mut record) } == 0 {
                return Ok(events);
            }
            let event = match record.kind {
                1 => Event::Close,
                2 => Event::Move {
                    x: record.x,
                    y: record.y,
                },
                3 => Event::Down {
                    x: record.x,
                    y: record.y,
                },
                4 => Event::Up {
                    x: record.x,
                    y: record.y,
                },
                5 => wheel(record.x, record.y, record.delta),
                6 => key(&record),
                7 => return Err(Error("native window event budget exhausted".into())),
                _ => return Err(Error("unknown native window event".into())),
            };
            events.push(event);
        }
    }
}

fn key(record: &Record) -> Event {
    Event::Key {
        code: record.code,
        repeat: record.repeat != 0,
        flags: record.flags,
    }
}

fn wheel(x: f64, y: f64, delta: f64) -> Event {
    Event::Wheel { x, y, delta }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn plectron_window_destroy(window: *mut c_void);
        }
        // SAFETY: The owned native window is destroyed once on its platform thread.
        unsafe { plectron_window_destroy(self.native.as_ptr()) };
    }
}
