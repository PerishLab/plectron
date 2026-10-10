use super::{State, paint};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    UI::WindowsAndMessaging::{
        CREATESTRUCTW, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
        WM_CLOSE, WM_NCCREATE, WM_NCDESTROY, WM_PAINT,
    },
};

pub unsafe extern "system" fn callback(
    handle: HWND,
    message: u32,
    word: WPARAM,
    parameter: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        // SAFETY: Windows supplies a live CREATESTRUCTW during WM_NCCREATE; its parameter is our stable boxed State.
        let creation = unsafe { &*(parameter as *const CREATESTRUCTW) };
        // SAFETY: this owner-thread HWND reserves its user-data slot for the State retained by Window.
        unsafe {
            SetWindowLongPtrW(handle, GWLP_USERDATA, creation.lpCreateParams as isize);
        }
    }
    // SAFETY: the user-data slot is read only on the window's owning thread and is cleared before native destruction completes.
    let pointer = unsafe { GetWindowLongPtrW(handle, GWLP_USERDATA) } as *const State;
    if !pointer.is_null() {
        // SAFETY: Window retains the boxed State throughout all synchronous native callbacks, including destruction.
        let state = unsafe { &*pointer };
        if let Some(result) = dispatch(handle, message, state) {
            return result;
        }
    }
    // SAFETY: unhandled messages are forwarded with the original Windows-provided arguments on the owner thread.
    unsafe { DefWindowProcW(handle, message, word, parameter) }
}

fn dispatch(handle: HWND, message: u32, state: &State) -> Option<LRESULT> {
    match message {
        WM_CLOSE => {
            state.closing.set(true);
            Some(0)
        }
        WM_PAINT => {
            if let Err(fault) = paint::draw(handle, state) {
                state.fault.replace(Some(fault));
            }
            Some(0)
        }
        WM_NCDESTROY => {
            state.destroyed.set(true);
            state.closing.set(true);
            // SAFETY: clearing the slot prevents callbacks from observing state after Window releases its box.
            unsafe {
                SetWindowLongPtrW(handle, GWLP_USERDATA, 0);
            }
            None
        }
        _ => None,
    }
}
