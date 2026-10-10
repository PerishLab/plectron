use super::{Error, error};
use graphics::shared::Pacing;
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    sync::Arc,
};
use windows_sys::Win32::{
    Foundation::{HANDLE, WAIT_FAILED},
    System::Threading::{CreateEventW, SetEvent},
    UI::WindowsAndMessaging::{
        DispatchMessageW, MSG, MWMO_INPUTAVAILABLE, MsgWaitForMultipleObjectsEx, PM_REMOVE,
        PeekMessageW, QS_ALLINPUT, TranslateMessage,
    },
};

pub(super) struct Events(Arc<OwnedHandle>);
impl Events {
    pub fn new() -> Result<Self, Error> {
        // SAFETY: an unnamed auto-reset event is created without inherited handles; the return transfers one owned handle.
        let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        if handle.is_null() {
            return Err(error(std::io::Error::last_os_error()));
        }
        // SAFETY: CreateEventW succeeded and returned a uniquely owned kernel handle.
        Ok(Self(Arc::new(unsafe {
            OwnedHandle::from_raw_handle(handle)
        })))
    }
    pub fn notifier(&self) -> shared::Notify {
        let handle = self.0.clone();
        shared::Notify::new(move || {
            // SAFETY: the Arc retains the event handle across all notifications; SetEvent is permitted across threads.
            unsafe {
                SetEvent(handle.as_raw_handle());
            }
        })
    }
    pub fn wait(&self, pacing: Pacing) -> Result<(), Error> {
        let timeout = match pacing {
            Pacing::Idle => u32::MAX,
            Pacing::Continuous => 16,
            Pacing::At(deadline) => deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_millis()
                .saturating_add(1)
                .min((u32::MAX - 1) as u128) as u32,
        };
        let handle: HANDLE = self.0.as_raw_handle();
        // SAFETY: the one-element handle array stays live for the wait; message delivery remains on the UI owner thread.
        if unsafe {
            MsgWaitForMultipleObjectsEx(1, &handle, timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE)
        } == WAIT_FAILED
        {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(())
    }
    pub fn poll(&self) -> Result<(), Error> {
        let mut message = MSG::default();
        for _ in 0..256 {
            // SAFETY: message is a valid writable MSG; null selects the current thread's queue, with bounded removal.
            if unsafe { PeekMessageW(&mut message, std::ptr::null_mut(), 0, 0, PM_REMOVE) } == 0 {
                break;
            }
            // SAFETY: message was returned by PeekMessageW and is dispatched on the queue's owning thread.
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn wake() {
        let events = Events::new().unwrap();
        let notifier = events.notifier();
        let worker = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            notifier.notify();
        });
        let started = Instant::now();
        events.wait(Pacing::Idle).unwrap();
        assert!(started.elapsed() < Duration::from_secs(1));
        worker.join().unwrap();
        let started = Instant::now();
        events
            .wait(Pacing::At(started + Duration::from_millis(20)))
            .unwrap();
        assert!(started.elapsed() >= Duration::from_millis(15));
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
