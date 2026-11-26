use super::{Error, State, error};
use crate::shared::{Extent, Image};
use std::ptr::NonNull;
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACKNESS, BeginPaint, BitBlt, CreateCompatibleDC,
        CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, EndPaint, HBITMAP, HDC, HGDIOBJ,
        PAINTSTRUCT, PatBlt, SRCCOPY, SelectObject, StretchBlt,
    },
    UI::WindowsAndMessaging::GetClientRect,
};

pub(super) struct Surface {
    device: HDC,
    bitmap: HBITMAP,
    prior: HGDIOBJ,
    bits: NonNull<u32>,
    width: usize,
    height: usize,
}

impl Surface {
    fn new(width: usize, height: usize) -> Result<Self, Error> {
        let count = width
            .checked_mul(height)
            .filter(|count| *count > 0)
            .ok_or_else(|| Error("native surface dimensions are invalid".into()))?;
        let _ = count;
        // SAFETY: null creates a screen-compatible memory DC owned by this Surface until DeleteDC in Drop.
        let device = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
        if device.is_null() {
            return Err(error(std::io::Error::last_os_error()));
        }
        let information = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut raw = std::ptr::null_mut();
        // SAFETY: device is a live memory DC and information describes a top-down 32-bit BI_RGB DIBSection.
        let bitmap = unsafe {
            CreateDIBSection(
                device,
                &information,
                DIB_RGB_COLORS,
                &mut raw,
                std::ptr::null_mut(),
                0,
            )
        };
        let Some(bits) = (!bitmap.is_null())
            .then(|| NonNull::new(raw.cast::<u32>()))
            .flatten()
        else {
            let fault = error(std::io::Error::last_os_error());
            // SAFETY: cleanup releases the locally created bitmap and DC on allocation failure.
            unsafe {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
                DeleteDC(device);
            }
            return Err(fault);
        };
        // SAFETY: bitmap is owned by this Surface and selected into its compatible memory DC.
        let prior = unsafe { SelectObject(device, bitmap) };
        if prior.is_null() {
            let fault = error(std::io::Error::last_os_error());
            // SAFETY: bitmap was not selected; release both owned GDI objects.
            unsafe {
                DeleteObject(bitmap);
                DeleteDC(device);
            }
            return Err(fault);
        }
        Ok(Self {
            device,
            bitmap,
            prior,
            bits,
            width,
            height,
        })
    }

    pub fn extent(&self) -> Extent {
        Extent {
            width: self.width,
            height: self.height,
        }
    }

    fn copy(&mut self, pixels: &[u32]) {
        // SAFETY: pixels length equals width * height of the owned DIBSection backing buffer and does not overlap bits.
        unsafe {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), self.bits.as_ptr(), pixels.len());
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: prior was the original bitmap in device; restoring it lets DeleteObject and DeleteDC release owned GDI handles cleanly.
        unsafe {
            SelectObject(self.device, self.prior);
            DeleteObject(self.bitmap);
            DeleteDC(self.device);
        }
    }
}

pub(super) fn upload(state: &State, image: &Image) -> Result<(), Error> {
    let mut slot = state.surface.borrow_mut();
    let rebuild = slot
        .as_ref()
        .is_none_or(|surface| surface.extent() != image.extent);
    if rebuild {
        *slot = Some(Surface::new(image.extent.width, image.extent.height)?);
    }
    if let Some(surface) = slot.as_mut() {
        surface.copy(&image.pixels);
    }
    Ok(())
}

pub(super) fn draw(handle: HWND, state: &State) -> Result<(), Error> {
    let mut paint = PAINTSTRUCT::default();
    // SAFETY: the live owner-thread HWND receives a writable PAINTSTRUCT, retained until its paired EndPaint.
    let device = unsafe { BeginPaint(handle, &mut paint) };
    if device.is_null() {
        return Err(error(std::io::Error::last_os_error()));
    }
    let result = blit(handle, device, state);
    // SAFETY: paint is the structure returned by the successful BeginPaint for this same HWND.
    unsafe {
        EndPaint(handle, &paint);
    }
    result
}

fn blit(handle: HWND, device: HDC, state: &State) -> Result<(), Error> {
    let mut bounds = RECT::default();
    // SAFETY: handle is live on its owner thread and bounds is writable.
    if unsafe { GetClientRect(handle, &mut bounds) } == 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    if width <= 0 || height <= 0 {
        return Ok(());
    }
    let slot = state.surface.borrow();
    let Some(surface) = slot.as_ref() else {
        // SAFETY: device is the active paint DC and the destination is its current client rectangle.
        unsafe {
            PatBlt(device, 0, 0, width, height, BLACKNESS);
        }
        return Ok(());
    };
    let mapped = state.mapped.get();
    if mapped && (surface.width < width as usize || surface.height < height as usize) {
        // SAFETY: the active paint DC clips this background clear to the live client rectangle before blitting the retained surface.
        unsafe {
            PatBlt(device, 0, 0, width, height, BLACKNESS);
        }
    }
    let ok = if mapped || (surface.width as i32 == width && surface.height as i32 == height) {
        // SAFETY: device is the active paint DC and surface.device holds the selected live DIBSection of size surface.width x surface.height.
        unsafe {
            BitBlt(
                device,
                0,
                0,
                surface.width as i32,
                surface.height as i32,
                surface.device,
                0,
                0,
                SRCCOPY,
            )
        }
    } else {
        // SAFETY: device is the active paint DC and surface.device holds the selected live DIBSection stretched to the client rectangle.
        unsafe {
            StretchBlt(
                device,
                0,
                0,
                width,
                height,
                surface.device,
                0,
                0,
                surface.width as i32,
                surface.height as i32,
                SRCCOPY,
            )
        }
    };
    if ok == 0 {
        return Err(Error("native CPU bitmap presentation failed".into()));
    }
    Ok(())
}

impl super::Window {
    pub fn surface(
        &self,
        target: graphics::shared::Target,
        output: graphics::shared::Output,
    ) -> Result<graphics::d3d12::Surface, Error> {
        let _awareness = self.check()?;
        // SAFETY: self.handle is a live owner-thread HWND; Desktop closes and drops Surface before destroying Window.
        unsafe { graphics::d3d12::Surface::new(target, output, self.handle) }.map_err(error)
    }
}
