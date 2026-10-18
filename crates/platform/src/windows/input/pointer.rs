use super::{Error, Queue};
use crate::shared::{Button, Event, Phase, Point};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, WPARAM},
    Graphics::Gdi::ScreenToClient,
    UI::{
        Input::KeyboardAndMouse::{GetCapture, ReleaseCapture, SetCapture, SetFocus},
        WindowsAndMessaging::*,
    },
};

pub(super) fn dispatch(
    queue: &Queue,
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
        if let Some(position) = position(queue, point) {
            queue.push(Event::Wheel {
                position,
                delta: -((data.0 >> 16) as i16 as i64),
            })?;
        }
        return Ok(true);
    }
    let phase = match message {
        WM_LBUTTONDOWN => {
            queue.captured.set(true);
            // SAFETY: capture and focus are assigned to this live owner-thread HWND; no queue borrow is held across reentrant callbacks.
            unsafe {
                SetCapture(handle);
                SetFocus(handle);
            }
            Phase::Down(Button::Primary)
        }
        WM_LBUTTONUP if inside(handle, point)? => Phase::Up(Button::Primary),
        WM_LBUTTONUP => Phase::Cancel,
        _ => Phase::Move,
    };
    queue.push(Event::Pointer {
        position: position(queue, point),
        phase,
    })?;
    if message == WM_LBUTTONUP {
        queue.captured.set(false);
        release(handle);
    }
    Ok(true)
}
fn position(queue: &Queue, point: POINT) -> Option<Point> {
    let dpi = u64::from(queue.density.get().unwrap_or(96));
    Some(Point {
        x: i64::try_from(u64::try_from(point.x).ok()? * 96 / dpi).ok()?,
        y: i64::try_from(u64::try_from(point.y).ok()? * 96 / dpi).ok()?,
    })
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
    let mut bounds = windows_sys::Win32::Foundation::RECT::default();
    // SAFETY: bounds is writable and this callback's HWND remains live on its owner thread.
    if unsafe { GetClientRect(handle, &mut bounds) } == 0 {
        return Err(super::super::error(std::io::Error::last_os_error()));
    }
    Ok((0..bounds.right).contains(&point.x) && (0..bounds.bottom).contains(&point.y))
}
