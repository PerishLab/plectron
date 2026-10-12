use super::{Error, error, input::Queue};
use crate::shared::{Event, Extent};
use graphics::shared::Output;
use std::cell::Cell;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, RECT, WPARAM},
    UI::{HiDpi::*, WindowsAndMessaging::*},
};

pub(super) struct Awareness(DPI_AWARENESS_CONTEXT);
impl Awareness {
    pub fn new() -> Result<Self, Error> {
        // SAFETY: changes only this owner thread's awareness; the returned previous context is restored by this guard.
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.is_null() {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(Self(previous))
    }
}
impl Drop for Awareness {
    fn drop(&mut self) {
        // SAFETY: this non-Send guard remains on its creating thread; the saved context was returned by the same API.
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}
pub(super) struct View {
    dpi: Cell<u32>,
    output: Cell<Option<Output>>,
    changed: Cell<bool>,
}
impl Default for View {
    fn default() -> Self {
        Self {
            dpi: Cell::new(96),
            output: Cell::new(None),
            changed: Cell::new(false),
        }
    }
}
impl View {
    pub fn scale(&self) -> f64 {
        f64::from(self.dpi.get()) / 96.0
    }
    pub fn changed(&self) -> bool {
        self.changed.replace(false)
    }
    pub fn initialize(&self, handle: HWND, extent: Extent) -> Result<(), Error> {
        // SAFETY: this HWND is live and belongs to the current per-monitor-aware thread.
        let dpi = unsafe { GetDpiForWindow(handle) };
        if dpi == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        self.dpi.set(dpi);
        let output = Output::new(
            graphics::shared::Extent {
                width: extent.width,
                height: extent.height,
            },
            self.scale(),
        )
        .map_err(error)?;
        let pixels = output.pixels();
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: pixels.width as i32,
            bottom: pixels.height as i32,
        };
        // SAFETY: writable bounds describe the desired client pixels and the owned top-level window's exact styles.
        if unsafe { AdjustWindowRectExForDpi(&mut bounds, WS_OVERLAPPEDWINDOW, 0, 0, dpi) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        // SAFETY: resize only this live owned window; callbacks observe initialized DPI and stable boxed state.
        if unsafe {
            SetWindowPos(
                handle,
                std::ptr::null_mut(),
                0,
                0,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            )
        } == 0
        {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(())
    }
    pub fn dispatch(
        &self,
        handle: HWND,
        message: u32,
        data: (WPARAM, LPARAM),
        queue: &Queue,
    ) -> Result<bool, Error> {
        match message {
            WM_SIZE if data.0 != SIZE_MINIMIZED as usize => {
                self.sync(handle, queue)?;
                Ok(true)
            }
            WM_DPICHANGED => {
                let dpi = (data.0 & 0xffff) as u32;
                if dpi == 0 || dpi != (data.0 >> 16) as u32 || data.1 == 0 {
                    return Err(Error("invalid native DPI change".into()));
                }
                self.dpi.set(dpi);
                queue.density(dpi);
                // SAFETY: Windows provides a live suggested RECT during WM_DPICHANGED; copy it before synchronous reentrant resize.
                let bounds = unsafe { *(data.1 as *const RECT) };
                // SAFETY: the copied recommended bounds resize this owner-thread HWND while its state remains live.
                if unsafe {
                    SetWindowPos(
                        handle,
                        std::ptr::null_mut(),
                        bounds.left,
                        bounds.top,
                        bounds.right - bounds.left,
                        bounds.bottom - bounds.top,
                        SWP_NOZORDER | SWP_NOACTIVATE,
                    )
                } == 0
                {
                    return Err(error(std::io::Error::last_os_error()));
                }
                self.sync(handle, queue)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }
    fn sync(&self, handle: HWND, queue: &Queue) -> Result<(), Error> {
        let mut bounds = RECT::default();
        // SAFETY: bounds is writable and this HWND belongs to the callback's owner thread.
        if unsafe { GetClientRect(handle, &mut bounds) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        if bounds.right <= 0 || bounds.bottom <= 0 {
            return Ok(());
        }
        let logical = graphics::shared::Extent {
            width: (bounds.right as u64 * 96)
                .div_ceil(u64::from(self.dpi.get()))
                .max(1) as usize,
            height: (bounds.bottom as u64 * 96)
                .div_ceil(u64::from(self.dpi.get()))
                .max(1) as usize,
        };
        let output = Output::new(logical, self.scale()).map_err(error)?;
        queue.density(self.dpi.get());
        if self.output.replace(Some(output)) != Some(output) {
            self.changed.set(true);
            queue.push(Event::Viewport {
                width: logical.width,
                height: logical.height,
                density: (self.dpi.get(), 96),
            })?;
        }
        Ok(())
    }
}
