mod paint;
pub(super) mod procedure;
use super::{Error, class::Class, error};
use crate::shared::{Image, Window as Options};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{InvalidateRect, UpdateWindow},
    UI::WindowsAndMessaging::{
        AdjustWindowRectEx, CW_USEDEFAULT, CreateWindowExW, DestroyWindow, SW_SHOW, SetWindowTextW,
        ShowWindow, WS_OVERLAPPEDWINDOW,
    },
};

pub(super) struct State {
    image: RefCell<Option<Image>>,
    closing: Cell<bool>,
    destroyed: Cell<bool>,
    fault: RefCell<Option<Error>>,
}
pub(super) struct Window {
    handle: HWND,
    class: Rc<Class>,
    state: Box<State>,
}
impl Window {
    pub fn new(class: Rc<Class>, options: &Options) -> Result<Self, Error> {
        class.check()?;
        options.extent.area().map_err(error)?;
        let title = U16CString::from_str(&options.title).map_err(error)?;
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: options.extent.width as i32,
            bottom: options.extent.height as i32,
        };
        // SAFETY: bounds is writable; the styles match the subsequently created top-level window without a menu.
        if unsafe { AdjustWindowRectEx(&mut bounds, WS_OVERLAPPEDWINDOW, 0, 0) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        let mut state = Box::new(State {
            image: RefCell::new(None),
            closing: Cell::new(false),
            destroyed: Cell::new(false),
            fault: RefCell::new(None),
        });
        // SAFETY: class and strings stay live; the boxed state has a stable address until DestroyWindow completes on this thread.
        let handle = unsafe {
            CreateWindowExW(
                0,
                class.name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                bounds.right - bounds.left,
                bounds.bottom - bounds.top,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                class.module,
                (&mut *state as *mut State).cast(),
            )
        };
        if handle.is_null() {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(Self {
            handle,
            class,
            state,
        })
    }
    fn check(&self) -> Result<(), Error> {
        self.class.check()?;
        if self.state.destroyed.get() {
            return Err(Error("native window was destroyed".into()));
        }
        Ok(())
    }
    pub fn show(&self) -> Result<(), Error> {
        self.check()?;
        // SAFETY: this live owned HWND is accessed on its owner thread; callbacks retain the boxed state.
        unsafe {
            ShowWindow(self.handle, SW_SHOW);
            UpdateWindow(self.handle);
        }
        self.result()
    }
    pub fn title(&self, title: &str) -> Result<(), Error> {
        self.check()?;
        let title = U16CString::from_str(title).map_err(error)?;
        // SAFETY: the HWND is live on this thread and the terminated UTF-16 title remains live for the call.
        if unsafe { SetWindowTextW(self.handle, title.as_ptr()) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(())
    }
    pub fn present(&self, image: Image) -> Result<(), Error> {
        self.check()?;
        if image.extent.area().map_err(error)? != image.pixels.len() {
            return Err(Error("native image dimensions disagree with pixels".into()));
        }
        self.state.image.replace(Some(image));
        // SAFETY: the HWND is live; image mutation ended before synchronous painting can borrow the retained pixels.
        if unsafe { InvalidateRect(self.handle, std::ptr::null(), 0) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        // SAFETY: painting executes on this window's owner thread with its boxed state retained.
        unsafe {
            UpdateWindow(self.handle);
        }
        self.result()
    }
    pub fn result(&self) -> Result<(), Error> {
        self.state.fault.borrow_mut().take().map_or(Ok(()), Err)
    }
    pub fn closing(&self) -> bool {
        self.state.closing.replace(false)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        if !self.state.destroyed.get() {
            // SAFETY: Rc keeps this owner-thread window non-Send; state and class remain live until synchronous destruction returns.
            unsafe {
                DestroyWindow(self.handle);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Extent;
    use windows_sys::Win32::{
        Graphics::Gdi::{GetDC, GetPixel, ReleaseDC},
        UI::WindowsAndMessaging::{GetClientRect, IsWindow, SendMessageW, WM_CLOSE},
    };

    #[test]
    fn pixels() {
        let window = Window::new(
            Rc::new(Class::new().unwrap()),
            &Options {
                title: "Plectron native bitmap probe".into(),
                extent: Extent {
                    width: 64,
                    height: 32,
                },
            },
        )
        .unwrap();
        window.show().unwrap();
        let pixels = (0..32)
            .flat_map(|_| (0..64).map(|x| if x < 32 { 0xff0000 } else { 0x0000ff }))
            .collect();
        window
            .present(Image {
                extent: Extent {
                    width: 64,
                    height: 32,
                },
                pixels,
            })
            .unwrap();
        // SAFETY: the live owner-thread HWND provides a borrowed DC, released after these synchronous pixel reads.
        let device = unsafe { GetDC(window.handle) };
        assert!(!device.is_null());
        // SAFETY: the borrowed DC is live and the coordinates lie within the window's validated client rectangle.
        let mut bounds = RECT::default();
        // SAFETY: this live owner-thread HWND receives a writable client rectangle.
        assert_ne!(unsafe { GetClientRect(window.handle, &mut bounds) }, 0);
        // SAFETY: both samples lie inside the current client rectangle, including native minimum-size adjustment.
        let colors = unsafe {
            (
                GetPixel(device, bounds.right / 4, bounds.bottom / 2),
                GetPixel(device, bounds.right * 3 / 4, bounds.bottom / 2),
            )
        };
        // SAFETY: device is the borrowed DC acquired from this same live HWND.
        unsafe {
            ReleaseDC(window.handle, device);
        }
        assert_eq!(colors, (0x0000ff, 0xff0000));
        assert!(
            window
                .present(Image {
                    extent: Extent {
                        width: 1,
                        height: 1
                    },
                    pixels: vec![]
                })
                .is_err()
        );
        let handle = window.handle;
        // SAFETY: close is dispatched synchronously to our live owner-thread window; State remains boxed throughout.
        unsafe {
            SendMessageW(handle, WM_CLOSE, 0, 0);
        }
        assert!(window.closing());
        assert!(!window.closing());
        drop(window);
        // SAFETY: IsWindow accepts an opaque former HWND and does not dereference Rust state.
        assert_eq!(unsafe { IsWindow(handle) }, 0);
    }
}
