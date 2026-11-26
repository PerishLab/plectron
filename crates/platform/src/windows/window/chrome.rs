use super::{Error, Stage, State, Window};
use crate::shared::{Chrome, Progress, Region};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT},
    Graphics::{
        Dwm::{DWMWA_CLOAK, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmSetWindowAttribute},
        Gdi::ScreenToClient,
    },
    UI::WindowsAndMessaging::{
        HTCAPTION, SW_SHOW, ShowWindow, WINDOW_STYLE, WS_MINIMIZEBOX, WS_OVERLAPPEDWINDOW,
        WS_POPUP, WS_SYSMENU,
    },
};

impl Window {
    pub(super) fn arm(&self) -> Result<(), Error> {
        let _awareness = self.check()?;
        if self.state.shown() {
            return Ok(());
        }
        if self.state.surface.borrow().is_some() {
            return self.reveal();
        }
        self.state.stage.set(Stage::Armed);
        Ok(())
    }

    pub(super) fn reveal(&self) -> Result<(), Error> {
        self.state.stage.set(Stage::Shown);
        cloak(self.handle, true);
        // SAFETY: this live owned HWND is shown while cloaked so the first paint completes before composition.
        unsafe {
            ShowWindow(self.handle, SW_SHOW);
        }
        let painted = self.repaint();
        cloak(self.handle, false);
        self.reapply();
        painted
    }

    pub(super) fn reapply(&self) {
        let progress = self.state.progress.get();
        if progress != Progress::None {
            self.state.taskbar.apply(self.handle, progress);
        }
    }

    pub fn drag(&self, regions: &[Region]) -> Result<(), Error> {
        let _awareness = self.check()?;
        let mut slot = self.state.drag.borrow_mut();
        slot.clear();
        slot.extend_from_slice(regions);
        Ok(())
    }

    pub fn progress(&self, progress: Progress) -> Result<(), Error> {
        let _awareness = self.check()?;
        self.state.progress.set(progress);
        if self.state.shown() {
            self.state.taskbar.apply(self.handle, progress);
        }
        Ok(())
    }
}

pub(super) fn style(chrome: Chrome) -> WINDOW_STYLE {
    match chrome {
        Chrome::Native => WS_OVERLAPPEDWINDOW,
        Chrome::Frameless => WS_POPUP | WS_MINIMIZEBOX | WS_SYSMENU,
    }
}

pub(super) fn round(handle: HWND, chrome: Chrome) {
    if chrome != Chrome::Frameless {
        return;
    }
    let preference = DWMWCP_ROUND;
    // SAFETY: handle is a live owner-thread HWND and preference points to a stack i32 of size 4; unsupported OS builds return an ignored HRESULT.
    let _ = unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_WINDOW_CORNER_PREFERENCE as u32,
            (&preference as *const i32).cast(),
            size_of::<i32>() as u32,
        )
    };
}

pub(super) fn cloak(handle: HWND, enabled: bool) {
    let value = i32::from(enabled);
    // SAFETY: handle is a live owner-thread HWND and value points to a stack i32 of size 4; non-DWM sessions return an ignored HRESULT.
    let _ = unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_CLOAK as u32,
            (&value as *const i32).cast(),
            size_of::<i32>() as u32,
        )
    };
}

pub(super) fn hit(handle: HWND, data: LPARAM, state: &State) -> Option<LRESULT> {
    let regions = state.drag.borrow();
    if regions.is_empty() {
        return None;
    }
    let mut point = POINT {
        x: data as i16 as i32,
        y: (data >> 16) as i16 as i32,
    };
    // SAFETY: point is writable and handle is a live owner-thread HWND.
    if unsafe { ScreenToClient(handle, &mut point) } == 0 {
        return None;
    }
    let dpi = i64::from(state.view.dpi());
    let x = (i64::from(point.x) * 96).div_euclid(dpi);
    let y = (i64::from(point.y) * 96).div_euclid(dpi);
    if x < 0 || y < 0 {
        return None;
    }
    let (x, y) = (x as usize, y as usize);
    regions
        .iter()
        .any(|region| region.contains(x, y))
        .then_some(HTCAPTION as LRESULT)
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use crate::shared::{Chrome, Extent, Progress, Region};
    use windows_sys::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{ClientToScreen, GetDC, GetPixel, ReleaseDC},
        UI::WindowsAndMessaging::{
            GetClientRect, GetWindowRect, HTCLIENT, IsWindowVisible, SendMessageW, WM_NCHITTEST,
        },
    };

    #[test]
    fn deferred() {
        let window = Window::new(
            Rc::new(Class::new().unwrap()),
            Rc::new(Taskbar::fallback()),
            &Options::new(
                "Plectron deferred reveal probe",
                Extent {
                    width: 64,
                    height: 32,
                },
            ),
        )
        .unwrap();
        // SAFETY: IsWindowVisible queries visibility of our live owner-thread HWND.
        assert_eq!(unsafe { IsWindowVisible(window.handle) }, 0);
        window.activate().unwrap();
        // SAFETY: window stays hidden while armed until first-frame pixels are installed.
        assert_eq!(unsafe { IsWindowVisible(window.handle) }, 0);
        assert!(!window.state.shown());
        let pixels = (0..32)
            .flat_map(|_| (0..64).map(|x| if x < 32 { 0x00ff00 } else { 0x0000ff }))
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
        assert!(window.state.shown());
        // SAFETY: window is now visible with first-frame pixels already blitted.
        assert_ne!(unsafe { IsWindowVisible(window.handle) }, 0);
        let _awareness = window.check().unwrap();
        // SAFETY: borrowed DC from our live owner-thread HWND, released below.
        let device = unsafe { GetDC(window.handle) };
        assert!(!device.is_null());
        let mut bounds = RECT::default();
        // SAFETY: bounds is writable for our live owner-thread HWND.
        assert_ne!(unsafe { GetClientRect(window.handle, &mut bounds) }, 0);
        // SAFETY: sampled points lie strictly inside the live client rectangle.
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
        assert_eq!(colors, (0x00ff00, 0xff0000));
    }

    #[test]
    fn frameless() {
        let window = Window::new(
            Rc::new(Class::new().unwrap()),
            Rc::new(Taskbar::fallback()),
            &Options {
                title: "Plectron frameless probe".into(),
                extent: Extent {
                    width: 240,
                    height: 160,
                },
                chrome: Chrome::Frameless,
            },
        )
        .unwrap();
        let _awareness = window.check().unwrap();
        let (mut outer, mut client) = (RECT::default(), RECT::default());
        // SAFETY: outer and client are writable for our live owner-thread HWND.
        assert_ne!(unsafe { GetWindowRect(window.handle, &mut outer) }, 0);
        // SAFETY: client is writable for our live owner-thread HWND.
        assert_ne!(unsafe { GetClientRect(window.handle, &mut client) }, 0);
        assert_eq!(outer.right - outer.left, client.right - client.left);
        assert_eq!(outer.bottom - outer.top, client.bottom - client.top);
        window.show().unwrap();
        window
            .drag(&[Region {
                left: 0,
                top: 0,
                right: 200,
                bottom: 32,
            }])
            .unwrap();
        let scale = window.state.view.scale();
        let query = |lx: i32, ly: i32| {
            let mut screen = POINT {
                x: (f64::from(lx) * scale).round() as i32,
                y: (f64::from(ly) * scale).round() as i32,
            };
            // SAFETY: screen is writable and handle is our live owner-thread HWND.
            assert_ne!(unsafe { ClientToScreen(window.handle, &mut screen) }, 0);
            let packed =
                ((screen.y as i16 as u16 as isize) << 16) | (screen.x as i16 as u16 as isize);
            // SAFETY: synchronous WM_NCHITTEST on our live owner-thread HWND while State stays boxed.
            unsafe { SendMessageW(window.handle, WM_NCHITTEST, 0, packed) }
        };
        assert_eq!(query(40, 12), HTCAPTION as LRESULT);
        assert_eq!(query(220, 12), HTCLIENT as LRESULT);
        assert_eq!(query(40, 80), HTCLIENT as LRESULT);
        window
            .progress(Progress::Normal {
                current: 50,
                total: 100,
            })
            .unwrap();
        assert_eq!(
            window.state.progress.get(),
            Progress::Normal {
                current: 50,
                total: 100
            }
        );
    }
}
