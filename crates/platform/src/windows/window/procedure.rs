use super::{Error, State, Window, chrome, paint};
use crate::shared::{Focus, Progress};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, WPARAM},
    Graphics::Gdi::ValidateRect,
    UI::WindowsAndMessaging::{
        CREATESTRUCTW, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, SetWindowLongPtrW,
        WM_CLOSE, WM_ERASEBKGND, WM_IME_SETCONTEXT, WM_NCCREATE, WM_NCDESTROY, WM_NCHITTEST,
        WM_PAINT,
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
        if let Some(result) = dispatch(handle, message, (word, parameter), state) {
            return result;
        }
    }
    // SAFETY: unhandled messages are forwarded with the original Windows-provided arguments on the owner thread.
    unsafe { DefWindowProcW(handle, message, word, parameter) }
}

fn dispatch(handle: HWND, message: u32, data: (WPARAM, LPARAM), state: &State) -> Option<LRESULT> {
    match state.view.dispatch(handle, message, data, &state.input) {
        Ok(true) => return Some(0),
        Err(fault) => {
            state.fault.replace(Some(fault));
            return Some(0);
        }
        Ok(false) => {}
    }
    if state.created != 0 && message == state.created {
        let progress = state.progress.get();
        if progress != Progress::None {
            state.taskbar.apply(handle, progress);
        }
        return Some(0);
    }
    match message {
        WM_ERASEBKGND => Some(1),
        WM_NCHITTEST => chrome::hit(handle, data.1, state),
        WM_IME_SETCONTEXT => Some(super::super::input::context(handle, data)),
        WM_CLOSE => {
            state.input.close();
            state.closing.set(true);
            Some(0)
        }
        WM_PAINT => repaint(handle, state),
        WM_NCDESTROY => {
            state.destroyed.set(true);
            state.input.close();
            state.closing.set(true);
            // SAFETY: clearing the slot prevents callbacks from observing state after Window releases its box.
            unsafe {
                SetWindowLongPtrW(handle, GWLP_USERDATA, 0);
            }
            None
        }
        _ => match state.input.dispatch(handle, message, data) {
            Ok(true) => Some(0),
            Ok(false) => None,
            Err(fault) => {
                state.fault.replace(Some(fault));
                Some(0)
            }
        },
    }
}

fn repaint(handle: HWND, state: &State) -> Option<LRESULT> {
    if state.accelerated.get() {
        // SAFETY: handle is a live owner-thread HWND owned by a DXGI Flip-Model swapchain.
        unsafe {
            ValidateRect(handle, std::ptr::null());
        }
        return Some(0);
    }
    if let Err(fault) = paint::draw(handle, state) {
        state.fault.replace(Some(fault));
    }
    Some(0)
}

impl Window {
    pub fn focus(&self, context: Option<&Focus>) -> Result<(), Error> {
        let _awareness = self.check()?;
        self.state
            .input
            .focus(self.handle, self.state.view.dpi(), context)
    }
}
