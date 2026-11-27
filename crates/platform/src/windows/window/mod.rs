mod abi;
mod chrome;
mod dialog;
mod frame;
#[cfg(test)]
mod input;
mod paint;
pub(super) mod procedure;
mod sighting;
use super::{Error, class::Class, error, taskbar, taskbar::Taskbar, timing::Sample};
use crate::shared::{Image, Progress, Region, Window as Options};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Instant;
use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{GdiFlush, InvalidateRect, UpdateWindow},
    UI::WindowsAndMessaging::{
        AdjustWindowRectEx, CW_USEDEFAULT, CreateWindowExW, DestroyWindow, SetWindowTextW,
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Stage {
    Hidden,
    Armed,
    Shown,
}

pub(super) struct State {
    surface: RefCell<Option<paint::Surface>>,
    mapped: Cell<bool>,
    accelerated: Cell<bool>,
    stage: Cell<Stage>,
    drag: RefCell<Vec<Region>>,
    progress: Cell<Progress>,
    taskbar: Rc<Taskbar>,
    created: u32,
    raster: RefCell<frame::Raster>,
    sightings: RefCell<sighting::Sightings>,
    view: super::viewport::View,
    input: super::input::Queue,
    closing: Cell<bool>,
    destroyed: Cell<bool>,
    fault: RefCell<Option<Error>>,
}

impl State {
    pub(super) fn shown(&self) -> bool {
        self.stage.get() == Stage::Shown
    }
}
pub(super) struct Window {
    handle: HWND,
    class: Rc<Class>,
    state: Box<State>,
}
impl Window {
    pub fn new(class: Rc<Class>, taskbar: Rc<Taskbar>, options: &Options) -> Result<Self, Error> {
        class.check()?;
        let _awareness = super::viewport::Awareness::new()?;
        options.extent.area().map_err(error)?;
        let title = U16CString::from_str(&options.title).map_err(error)?;
        let style = chrome::style(options.chrome);
        let mut bounds = RECT {
            left: 0,
            top: 0,
            right: options.extent.width as i32,
            bottom: options.extent.height as i32,
        };
        // SAFETY: bounds is writable; the styles match the subsequently created top-level window without a menu.
        if unsafe { AdjustWindowRectEx(&mut bounds, style, 0, 0) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        let mut state = Box::new(State {
            surface: RefCell::new(None),
            mapped: Cell::new(false),
            accelerated: Cell::new(false),
            stage: Cell::new(Stage::Hidden),
            drag: RefCell::new(Vec::new()),
            progress: Cell::new(Progress::None),
            taskbar,
            created: taskbar::registered(),
            raster: RefCell::new(frame::Raster::default()),
            sightings: RefCell::new(sighting::Sightings::default()),
            view: super::viewport::View::default(),
            input: super::input::Queue::default(),
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
                style,
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
        let window = Self {
            handle,
            class,
            state,
        };
        window
            .state
            .view
            .initialize(handle, options.extent, style, &window.state.input)?;
        chrome::round(handle, options.chrome);
        Ok(window)
    }
    fn check(&self) -> Result<super::viewport::Awareness, Error> {
        self.class.check()?;
        if self.state.destroyed.get() {
            return Err(Error("native window was destroyed".into()));
        }
        super::viewport::Awareness::new()
    }
    #[cfg(test)]
    pub fn show(&self) -> Result<(), Error> {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_SHOW, ShowWindow};
        let _awareness = self.check()?;
        self.state.stage.set(Stage::Shown);
        // SAFETY: this live owned HWND is accessed on its owner thread; callbacks retain the boxed state.
        unsafe {
            ShowWindow(self.handle, SW_SHOW);
            UpdateWindow(self.handle);
        }
        self.reapply();
        self.result()
    }
    pub fn activate(&self) -> Result<(), Error> {
        self.arm()
    }
    pub fn title(&self, title: &str) -> Result<(), Error> {
        let _awareness = self.check()?;
        let title = U16CString::from_str(title).map_err(error)?;
        // SAFETY: the HWND is live on this thread and the terminated UTF-16 title remains live for the call.
        if unsafe { SetWindowTextW(self.handle, title.as_ptr()) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        Ok(())
    }
    pub fn present(&self, image: Image) -> Result<(), Error> {
        self.state.raster.borrow_mut().clear();
        self.install(image, false)
    }
    fn install(&self, image: Image, mapped: bool) -> Result<(), Error> {
        let _awareness = self.check()?;
        if image.extent.area().map_err(error)? != image.pixels.len() {
            return Err(Error("native image dimensions disagree with pixels".into()));
        }
        self.state.mapped.set(mapped);
        paint::upload(&self.state, &image)?;
        if self.state.stage.get() == Stage::Armed {
            return self.reveal();
        }
        self.repaint()
    }
    fn repaint(&self) -> Result<(), Error> {
        // SAFETY: the HWND is live; surface mutation ended before synchronous painting can borrow the retained surface.
        if unsafe { InvalidateRect(self.handle, std::ptr::null(), 0) } == 0 {
            return Err(error(std::io::Error::last_os_error()));
        }
        // SAFETY: painting executes on this window's owner thread with its boxed state retained; GdiFlush commits queued blits.
        unsafe {
            UpdateWindow(self.handle);
            GdiFlush();
        }
        self.result()
    }
    pub fn sight(&self, sample: Sample) {
        self.state
            .sightings
            .borrow_mut()
            .settle(sample.refresh, sample.vblank, self.state.shown());
    }
    pub fn sighted(&self) -> Vec<(u64, Instant)> {
        self.state.sightings.borrow_mut().take()
    }
    pub fn awaiting(&self) -> bool {
        self.state.sightings.borrow().awaiting()
    }
    pub fn result(&self) -> Result<(), Error> {
        self.state.fault.borrow_mut().take().map_or(Ok(()), Err)
    }
    pub fn input(&self) -> Vec<crate::shared::Event> {
        self.state.input.drain()
    }
    pub fn pending(&self) -> bool {
        self.state.input.pending()
    }
    pub fn closing(&self) -> bool {
        self.state.closing.replace(false)
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.state.sightings.borrow_mut().clear();
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
        let extent = Extent {
            width: 64,
            height: 32,
        };
        let window = Window::new(
            Rc::new(Class::new().unwrap()),
            Rc::new(Taskbar::fallback()),
            &Options::new("Plectron native bitmap probe", extent),
        )
        .unwrap();
        window.show().unwrap();
        let pixels = (0..32)
            .flat_map(|_| (0..64).map(|x| if x < 32 { 0xff0000 } else { 0x0000ff }))
            .collect();
        window.present(Image { extent, pixels }).unwrap();
        let _awareness = window.check().unwrap();
        // SAFETY: the live owner-thread HWND provides a borrowed DC, released after these synchronous pixel reads.
        let device = unsafe { GetDC(window.handle) };
        assert!(!device.is_null());
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
