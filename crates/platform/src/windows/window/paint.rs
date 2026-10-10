use super::{Error, State, error};
use windows_sys::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLACKNESS, BeginPaint, DIB_RGB_COLORS, EndPaint,
        GDI_ERROR, PAINTSTRUCT, PatBlt, SRCCOPY, StretchDIBits,
    },
    UI::WindowsAndMessaging::GetClientRect,
};

pub(super) fn draw(handle: HWND, state: &State) -> Result<(), Error> {
    let mut paint = PAINTSTRUCT::default();
    // SAFETY: the live owner-thread HWND receives a writable PAINTSTRUCT, retained until its paired EndPaint.
    let device = unsafe { BeginPaint(handle, &mut paint) };
    if device.is_null() {
        return Err(error(std::io::Error::last_os_error()));
    }
    let result = copy(handle, device, state);
    // SAFETY: paint is the structure returned by the successful BeginPaint for this same HWND.
    unsafe {
        EndPaint(handle, &paint);
    }
    result
}
fn copy(
    handle: HWND,
    device: windows_sys::Win32::Graphics::Gdi::HDC,
    state: &State,
) -> Result<(), Error> {
    let mut bounds = RECT::default();
    // SAFETY: handle is live on its owner thread and bounds is writable.
    if unsafe { GetClientRect(handle, &mut bounds) } == 0 {
        return Err(error(std::io::Error::last_os_error()));
    }
    let width = bounds.right - bounds.left;
    let height = bounds.bottom - bounds.top;
    if width == 0 || height == 0 {
        return Ok(());
    }
    let image = state.image.borrow();
    let Some(image) = image.as_ref() else {
        // SAFETY: device is the active paint DC and the destination is its current client rectangle.
        unsafe {
            PatBlt(device, 0, 0, width, height, BLACKNESS);
        }
        return Ok(());
    };
    let information = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: image.extent.width as i32,
            biHeight: -(image.extent.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: validated owned pixels remain immutably borrowed for this synchronous top-down 32-bit DIB copy; no pointer is retained.
    let copied = unsafe {
        StretchDIBits(
            device,
            0,
            0,
            width,
            height,
            0,
            0,
            image.extent.width as i32,
            image.extent.height as i32,
            image.pixels.as_ptr().cast(),
            &information,
            DIB_RGB_COLORS,
            SRCCOPY,
        )
    };
    if copied == 0 || copied == GDI_ERROR {
        return Err(Error("native CPU bitmap presentation failed".into()));
    }
    Ok(())
}
