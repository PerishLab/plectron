use super::{Error, error, window::procedure};
use std::sync::atomic::{AtomicU64, Ordering};
use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::HINSTANCE,
    System::{LibraryLoader::GetModuleHandleW, Threading::GetCurrentThreadId},
    UI::WindowsAndMessaging::{
        CS_HREDRAW, CS_VREDRAW, RegisterClassW, UnregisterClassW, WNDCLASSW,
    },
};

pub(super) struct Class {
    pub name: U16CString,
    pub module: HINSTANCE,
    owner: u32,
}
impl Class {
    pub fn new() -> Result<Self, Error> {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let name =
            U16CString::from_str(format!("Plectron-{}", NEXT.fetch_add(1, Ordering::Relaxed)))
                .map_err(error)?;
        // SAFETY: null requests the current executable module; the borrowed module remains loaded for this process.
        let module = unsafe { GetModuleHandleW(std::ptr::null()) };
        if module.is_null() {
            return Err(error(std::io::Error::last_os_error()));
        }
        let definition = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(procedure::callback),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: module,
            hIcon: std::ptr::null_mut(),
            hCursor: std::ptr::null_mut(),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: name.as_ptr(),
        };
        // SAFETY: definition and UTF-16 name are live; the callback remains loaded for every window of this class.
        if unsafe { RegisterClassW(&definition) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        // SAFETY: querying the current thread ID has no pointer or ownership requirements.
        let owner = unsafe { GetCurrentThreadId() };
        Ok(Self {
            name,
            module,
            owner,
        })
    }
    pub fn check(&self) -> Result<(), Error> {
        // SAFETY: querying the current thread ID has no pointer or ownership requirements.
        if unsafe { GetCurrentThreadId() } != self.owner {
            return Err(Error(
                "native window operation requires its owner thread".into(),
            ));
        }
        Ok(())
    }
}
impl Drop for Class {
    fn drop(&mut self) {
        // SAFETY: every owned window retains this class through Rc; the final owner runs after all windows are destroyed.
        unsafe {
            UnregisterClassW(self.name.as_ptr(), self.module);
        }
    }
}
