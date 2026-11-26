use crate::shared::Progress;
use std::ffi::c_void;
use windows_sys::{
    Win32::{
        Foundation::{HWND, RPC_E_CHANGED_MODE, S_FALSE, S_OK},
        System::Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoUninitialize,
        },
        UI::{
            Shell::{
                TBPF_ERROR, TBPF_INDETERMINATE, TBPF_NOPROGRESS, TBPF_NORMAL, TBPF_PAUSED, TBPFLAG,
                TaskbarList,
            },
            WindowsAndMessaging::RegisterWindowMessageW,
        },
    },
    core::{GUID, HRESULT},
};

const INTERFACE: GUID = GUID::from_u128(0xea1afb91_9e28_4b86_90e9_9e9f8a5eefaf);
const MESSAGE: [u16; 21] = [
    b'T' as u16,
    b'a' as u16,
    b's' as u16,
    b'k' as u16,
    b'b' as u16,
    b'a' as u16,
    b'r' as u16,
    b'B' as u16,
    b'u' as u16,
    b't' as u16,
    b't' as u16,
    b'o' as u16,
    b'n' as u16,
    b'C' as u16,
    b'r' as u16,
    b'e' as u16,
    b'a' as u16,
    b't' as u16,
    b'e' as u16,
    b'd' as u16,
    0,
];

#[repr(C)]
struct Table {
    query: *const c_void,
    add: *const c_void,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    init: unsafe extern "system" fn(*mut c_void) -> HRESULT,
    append: *const c_void,
    remove: *const c_void,
    activate: *const c_void,
    active: *const c_void,
    fullscreen: *const c_void,
    value: unsafe extern "system" fn(*mut c_void, HWND, u64, u64) -> HRESULT,
    state: unsafe extern "system" fn(*mut c_void, HWND, TBPFLAG) -> HRESULT,
}

pub(super) struct Taskbar {
    object: *mut c_void,
    com: bool,
}

impl Taskbar {
    pub fn new() -> Self {
        // SAFETY: initializes single-threaded COM on the UI owner thread; S_OK/S_FALSE are paired with CoUninitialize in Drop.
        let status = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        let com = status == S_OK || status == S_FALSE;
        if !com && status != RPC_E_CHANGED_MODE {
            return Self {
                object: std::ptr::null_mut(),
                com: false,
            };
        }
        let mut raw = std::ptr::null_mut();
        // SAFETY: CLSID_TaskbarList and IID_ITaskbarList3 are valid GUIDs and raw receives an owned COM pointer on success.
        let created = unsafe {
            CoCreateInstance(
                &TaskbarList,
                std::ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &INTERFACE,
                &mut raw,
            )
        };
        if created < 0 || raw.is_null() {
            return Self {
                object: std::ptr::null_mut(),
                com,
            };
        }
        // SAFETY: raw is a valid COM ITaskbarList3 object whose first word points to its immutable vtable matching Table.
        let table = unsafe { &**(raw as *const *const Table) };
        // SAFETY: HrInit is invoked once on the newly created ITaskbarList3 instance on its owner thread.
        if unsafe { (table.init)(raw) } < 0 {
            // SAFETY: releasing the owned COM pointer on initialization failure avoids leaking the instance.
            unsafe {
                (table.release)(raw);
            }
            return Self {
                object: std::ptr::null_mut(),
                com,
            };
        }
        Self { object: raw, com }
    }

    #[cfg(test)]
    pub fn fallback() -> Self {
        Self {
            object: std::ptr::null_mut(),
            com: false,
        }
    }

    pub fn apply(&self, handle: HWND, progress: Progress) {
        if self.object.is_null() || handle.is_null() {
            return;
        }
        let (flag, fraction) = encode(progress);
        // SAFETY: self.object is an initialized ITaskbarList3 owned by this UI thread; handle is a live HWND.
        let table = unsafe { &**(self.object as *const *const Table) };
        if let Some((current, total)) = fraction {
            // SAFETY: SetProgressValue receives our initialized COM instance and live HWND; negative HRESULTs degrade silently.
            let _ = unsafe { (table.value)(self.object, handle, current, total) };
        }
        // SAFETY: SetProgressState receives our initialized COM instance, live HWND and valid TBPFLAG.
        let _ = unsafe { (table.state)(self.object, handle, flag) };
    }
}

impl Drop for Taskbar {
    fn drop(&mut self) {
        if !self.object.is_null() {
            // SAFETY: self.object is released before CoUninitialize on the UI owner thread.
            unsafe {
                let table = &**(self.object as *const *const Table);
                (table.release)(self.object);
            }
            self.object = std::ptr::null_mut();
        }
        if self.com {
            // SAFETY: matches the earlier S_OK/S_FALSE CoInitializeEx call on this same UI thread.
            unsafe {
                CoUninitialize();
            }
        }
    }
}

pub(super) fn registered() -> u32 {
    // SAFETY: MESSAGE is a static NUL-terminated UTF-16 string.
    unsafe { RegisterWindowMessageW(MESSAGE.as_ptr()) }
}

fn encode(progress: Progress) -> (TBPFLAG, Option<(u64, u64)>) {
    match progress {
        Progress::None => (TBPF_NOPROGRESS, None),
        Progress::Indeterminate => (TBPF_INDETERMINATE, None),
        Progress::Normal { current, total } => clamp(TBPF_NORMAL, current, total),
        Progress::Paused { current, total } => clamp(TBPF_PAUSED, current, total),
        Progress::Error { current, total } => clamp(TBPF_ERROR, current, total),
    }
}

fn clamp(flag: TBPFLAG, current: u64, total: u64) -> (TBPFLAG, Option<(u64, u64)>) {
    if total == 0 {
        (flag, Some((0, 1)))
    } else {
        (flag, Some((current.min(total), total)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encoding() {
        assert_eq!(encode(Progress::None), (TBPF_NOPROGRESS, None));
        assert_eq!(encode(Progress::Indeterminate), (TBPF_INDETERMINATE, None));
        assert_eq!(
            encode(Progress::Normal {
                current: 45,
                total: 100
            }),
            (TBPF_NORMAL, Some((45, 100)))
        );
        assert_eq!(
            encode(Progress::Paused {
                current: 150,
                total: 100
            }),
            (TBPF_PAUSED, Some((100, 100)))
        );
        assert_eq!(
            encode(Progress::Error {
                current: 3,
                total: 0
            }),
            (TBPF_ERROR, Some((0, 1)))
        );
    }

    #[test]
    fn fallback() {
        let taskbar = Taskbar::fallback();
        taskbar.apply(std::ptr::null_mut(), Progress::Indeterminate);
        taskbar.apply(
            std::ptr::null_mut(),
            Progress::Normal {
                current: 1,
                total: 2,
            },
        );
        let native = Taskbar::new();
        native.apply(std::ptr::null_mut(), Progress::None);
    }
}
