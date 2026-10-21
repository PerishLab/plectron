use super::{Error, Queue};
use crate::shared::{Button, Event, Phase, Point};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, RECT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Input::KeyboardAndMouse::{
            GetCapture, ReleaseCapture, SetCapture, SetFocus, TME_LEAVE, TRACKMOUSEEVENT,
            TrackMouseEvent,
        },
        WindowsAndMessaging::*,
    },
};

impl Queue {
    pub(super) fn pointer(
        &self,
        handle: HWND,
        message: u32,
        data: (WPARAM, LPARAM),
    ) -> Result<bool, Error> {
        let mut point = POINT {
            x: data.1 as i16 as i32,
            y: (data.1 >> 16) as i16 as i32,
        };
        if message == WM_MOUSEWHEEL {
            // SAFETY: point is writable and the live window belongs to the callback thread.
            if unsafe { ScreenToClient(handle, &mut point) } == 0 {
                return Err(super::super::error(std::io::Error::last_os_error()));
            }
            self.push(Event::Wheel {
                position: self.position(point),
                delta: -((data.0 >> 16) as i16 as i64),
            })?;
            return Ok(true);
        }
        let inside = inside(handle, point)?;
        let phase = match message {
            WM_LBUTTONDOWN => {
                self.captured.set(true);
                // SAFETY: capture and focus are assigned to this live owner-thread HWND; no queue borrow is held across reentrant callbacks.
                unsafe {
                    SetCapture(handle);
                    SetFocus(handle);
                }
                Phase::Down(Button::Primary)
            }
            WM_LBUTTONUP if self.captured.get() => Phase::Up(Button::Primary),
            WM_LBUTTONUP => return Ok(true),
            _ => Phase::Move,
        };
        if !self.captured.get() {
            self.track(handle);
        }
        self.outside.set(!inside);
        if inside {
            self.left.set(false);
        }
        self.push(Event::Pointer {
            position: Some(self.position(point)),
            phase,
        })?;
        if message == WM_LBUTTONUP {
            self.captured.set(false);
            release(handle);
            self.exit()?;
        }
        Ok(true)
    }

    /// A `WM_MOUSELEAVE` ends the tracking session. Outside a capture it reports the
    /// leave once the cursor is off the client area; Chromium likewise drops a leave
    /// while the cursor is still over its window. During a capture the last pointer
    /// position decides the leave when the capture ends.
    pub(super) fn leave(&self, handle: HWND) -> Result<bool, Error> {
        self.tracking.set(false);
        if self.captured.get() || over(handle) {
            return Ok(true);
        }
        self.outside.set(true);
        self.exit()?;
        Ok(true)
    }

    /// Windows posts `WM_MOUSELEAVE` only after `TrackMouseEvent` arms it, once per
    /// session, so an uncaptured move arms it while no session is open, as Chromium's
    /// `HWNDMessageHandler::TrackMouseEvents` does.
    fn track(&self, handle: HWND) {
        if self.tracking.replace(true) {
            return;
        }
        let mut request = TRACKMOUSEEVENT {
            cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
            dwFlags: TME_LEAVE,
            hwndTrack: handle,
            dwHoverTime: 0,
        };
        // SAFETY: request is a writable, correctly sized record naming this live owner-thread HWND.
        if unsafe { TrackMouseEvent(&mut request) } == 0 {
            self.tracking.set(false);
        }
    }

    fn position(&self, point: POINT) -> Point {
        let dpi = i64::from(self.density.get().unwrap_or(96));
        Point {
            x: (i64::from(point.x) * 96).div_euclid(dpi),
            y: (i64::from(point.y) * 96).div_euclid(dpi),
        }
    }
}

pub(super) fn release(handle: HWND) {
    // SAFETY: only this window's capture is released on its owner thread; callers clear capture state before reentrancy.
    unsafe {
        if GetCapture() == handle {
            ReleaseCapture();
        }
    }
}

fn inside(handle: HWND, point: POINT) -> Result<bool, Error> {
    let mut bounds = RECT::default();
    // SAFETY: bounds is writable and this callback's HWND remains live on its owner thread.
    if unsafe { GetClientRect(handle, &mut bounds) } == 0 {
        return Err(super::super::error(std::io::Error::last_os_error()));
    }
    Ok((0..bounds.right).contains(&point.x) && (0..bounds.bottom).contains(&point.y))
}

/// Whether the cursor is over this window's client area. A failed query (for
/// example on a locked desktop) counts as off the window.
fn over(handle: HWND) -> bool {
    let mut cursor = POINT::default();
    // SAFETY: cursor is writable; the query reads the current desktop cursor position.
    if unsafe { GetCursorPos(&mut cursor) } == 0 {
        return false;
    }
    // SAFETY: WindowFromPoint takes the point by value and returns a handle without borrowing it.
    if unsafe { WindowFromPoint(cursor) } != handle {
        return false;
    }
    // SAFETY: cursor is writable and the live window belongs to the callback thread.
    if unsafe { ScreenToClient(handle, &mut cursor) } == 0 {
        return false;
    }
    inside(handle, cursor).unwrap_or(false)
}
