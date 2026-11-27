use crate::shared::Caret;
use std::{
    ffi::{CStr, c_void},
    sync::OnceLock,
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    UI::WindowsAndMessaging::{DefWindowProcW, WM_IME_SETCONTEXT},
};

pub(super) const COMPOSITION: u32 = 0x0008;
pub(super) const CURSOR: u32 = 0x0080;
pub(super) const RESULT: u32 = 0x0800;
const ANCHOR: u32 = 0x0002;
const CANDIDATE: u32 = 0x0040;
const EXCLUDE: u32 = 0x0080;
const NOTIFY: u32 = 0x0015;
const CANCEL: u32 = 0x0004;
const SHOW: isize = 0x8000_0000;
const LIMIT: usize = 65_536;

type Handle = *mut c_void;
type Context = unsafe extern "system" fn(HWND) -> Handle;
type Release = unsafe extern "system" fn(HWND, Handle) -> i32;
type Query = unsafe extern "system" fn(Handle, u32, *mut c_void, u32) -> i32;
type Place = unsafe extern "system" fn(Handle, *const Form) -> i32;
type Suggest = unsafe extern "system" fn(Handle, *const Window) -> i32;
type Signal = unsafe extern "system" fn(Handle, u32, u32, u32) -> i32;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Form {
    pub style: u32,
    pub point: POINT,
    pub area: RECT,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct Window {
    pub index: u32,
    pub style: u32,
    pub point: POINT,
    pub area: RECT,
}

struct Api {
    context: Context,
    release: Release,
    query: Query,
    place: Place,
    suggest: Suggest,
    signal: Signal,
}

static LIBRARY: OnceLock<Option<Api>> = OnceLock::new();

fn api() -> Option<&'static Api> {
    LIBRARY.get_or_init(load).as_ref()
}

#[cfg(test)]
pub(super) fn ready() -> bool {
    api().is_some()
}

fn load() -> Option<Api> {
    let name: Vec<u16> = "imm32.dll\0".encode_utf16().collect();
    // SAFETY: name is a NUL-terminated UTF-16 system DLL name; imm32.dll stays resident for process lifetime.
    let module = unsafe { LoadLibraryW(name.as_ptr()) };
    if module.is_null() {
        return None;
    }
    // SAFETY: each export name matches the documented Win32 imm32.dll ABI and function pointer signature.
    unsafe {
        Some(Api {
            context: symbol(module, c"ImmGetContext")?,
            release: symbol(module, c"ImmReleaseContext")?,
            query: symbol(module, c"ImmGetCompositionStringW")?,
            place: symbol(module, c"ImmSetCompositionWindow")?,
            suggest: symbol(module, c"ImmSetCandidateWindow")?,
            signal: symbol(module, c"ImmNotifyIME")?,
        })
    }
}

unsafe fn symbol<T: Copy>(module: Handle, name: &CStr) -> Option<T> {
    // SAFETY: module is a live HMODULE and name is a NUL-terminated ASCII export symbol.
    let address = unsafe { GetProcAddress(module, name.as_ptr().cast()) }?;
    // SAFETY: caller guarantees T is the pointer-sized extern "system" fn type of the named export.
    Some(unsafe { std::mem::transmute_copy(&address) })
}

pub(super) struct Guard {
    api: &'static Api,
    window: HWND,
    handle: Handle,
}

impl Guard {
    pub(super) fn open(window: HWND) -> Option<Self> {
        let api = api()?;
        // SAFETY: window is a live owner-thread HWND; ImmGetContext returns an owned HIMC or null.
        let handle = unsafe { (api.context)(window) };
        if handle.is_null() {
            return None;
        }
        Some(Self {
            api,
            window,
            handle,
        })
    }

    pub(super) fn string(&self, index: u32) -> Option<Vec<u16>> {
        // SAFETY: self.handle is a live HIMC; null buffer queries required byte length.
        let bytes = unsafe { (self.api.query)(self.handle, index, std::ptr::null_mut(), 0) };
        if bytes < 0 {
            return None;
        }
        let length = (bytes as usize / 2).min(LIMIT);
        let mut units = vec![0u16; length];
        if length > 0 {
            // SAFETY: units holds length u16 elements (length * 2 writable bytes) for this live HIMC.
            let written = unsafe {
                (self.api.query)(
                    self.handle,
                    index,
                    units.as_mut_ptr().cast(),
                    (length * 2) as u32,
                )
            };
            if written < 0 {
                return None;
            }
            units.truncate((written as usize / 2).min(length));
        }
        Some(units)
    }

    pub(super) fn cursor(&self) -> Option<usize> {
        // SAFETY: self.handle is a live HIMC; GCS_CURSORPOS returns a UTF-16 code unit offset or negative error.
        let offset = unsafe { (self.api.query)(self.handle, CURSOR, std::ptr::null_mut(), 0) };
        (offset >= 0).then_some(offset as usize)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        // SAFETY: self.handle was acquired from ImmGetContext for this same live owner-thread HWND.
        unsafe {
            (self.api.release)(self.window, self.handle);
        }
    }
}

pub(super) fn context(handle: HWND, data: (WPARAM, LPARAM)) -> LRESULT {
    // SAFETY: handle is a live owner-thread HWND; masking ISC_SHOWUICOMPOSITIONWINDOW hides only the OS composition box.
    unsafe { DefWindowProcW(handle, WM_IME_SETCONTEXT, data.0, data.1 & !SHOW) }
}

pub(super) fn place(handle: HWND, caret: Caret, dpi: u32) {
    let Some(guard) = Guard::open(handle) else {
        return;
    };
    let (form, mut candidate) = geometry(caret, dpi);
    // SAFETY: guard.handle is a live HIMC and form/candidate are valid initialized IMM32 records.
    unsafe {
        (guard.api.place)(guard.handle, &form);
        if (guard.api.suggest)(guard.handle, &candidate) == 0 {
            candidate.style = CANDIDATE;
            (guard.api.suggest)(guard.handle, &candidate);
        }
    }
}

pub(super) fn abort(handle: HWND) {
    let Some(guard) = Guard::open(handle) else {
        return;
    };
    // SAFETY: guard.handle is a live HIMC; NI_COMPOSITIONSTR + CPS_CANCEL cancels any active native composition.
    unsafe {
        (guard.api.signal)(guard.handle, NOTIFY, CANCEL, 0);
    }
}

pub(super) fn geometry(caret: Caret, dpi: u32) -> (Form, Window) {
    let scale = f64::from(dpi.max(96)) / 96.0;
    let left = (caret.x as f64 * scale).round() as i32;
    let top = (caret.y as f64 * scale).round() as i32;
    let width = (caret.width.max(1) as f64 * scale).round().max(1.0) as i32;
    let height = (caret.height.max(1) as f64 * scale).round().max(1.0) as i32;
    let area = RECT {
        left,
        top,
        right: left.saturating_add(width),
        bottom: top.saturating_add(height),
    };
    let form = Form {
        style: ANCHOR,
        point: POINT { x: left, y: top },
        area,
    };
    let candidate = Window {
        index: 0,
        style: EXCLUDE,
        point: POINT {
            x: left,
            y: area.bottom,
        },
        area,
    };
    (form, candidate)
}
