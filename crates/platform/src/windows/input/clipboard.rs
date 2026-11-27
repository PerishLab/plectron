use std::{ffi::c_void, sync::Mutex, thread, time::Duration};
use windows_sys::Win32::Foundation::HWND;

const UNICODE: u32 = 13;
const MOVEABLE: u32 = 0x0002;
const LIMIT: usize = 2_097_152;
const ATTEMPTS: usize = 5;

static GUARD: Mutex<()> = Mutex::new(());

pub(super) struct System;

impl crate::shared::Clipboard for System {
    fn read(&self) -> Result<Option<String>, String> {
        read()
    }

    fn write(&self, text: &str) -> Result<(), String> {
        write(text)
    }
}

struct Lock;

impl Lock {
    fn open() -> Result<Self, String> {
        for attempt in 0..ATTEMPTS {
            // SAFETY: null associates the clipboard lock with the current task until Drop calls CloseClipboard.
            if unsafe { OpenClipboard(std::ptr::null_mut()) } != 0 {
                return Ok(Self);
            }
            if attempt + 1 < ATTEMPTS {
                thread::sleep(Duration::from_millis(2));
            }
        }
        Err("native clipboard is busy".into())
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        // SAFETY: Lock is constructed only after OpenClipboard succeeded on this thread.
        unsafe {
            CloseClipboard();
        }
    }
}

struct Block(*mut c_void);

impl Block {
    fn allocate(bytes: usize) -> Result<Self, String> {
        // SAFETY: GMEM_MOVEABLE allocates a moveable global block owned by this guard until release or Drop.
        let handle = unsafe { GlobalAlloc(MOVEABLE, bytes) };
        if handle.is_null() {
            return Err("native clipboard allocation failed".into());
        }
        Ok(Self(handle))
    }

    fn raw(&self) -> *mut c_void {
        self.0
    }

    fn release(mut self) {
        self.0 = std::ptr::null_mut();
    }
}

impl Drop for Block {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: ownership was not transferred to SetClipboardData, so GlobalFree releases this allocation.
            unsafe {
                GlobalFree(self.0);
            }
        }
    }
}

pub(super) fn read() -> Result<Option<String>, String> {
    let _guard = GUARD.lock().expect("clipboard lock");
    // SAFETY: IsClipboardFormatAvailable queries only the availability of CF_UNICODETEXT without borrowed pointers.
    if unsafe { IsClipboardFormatAvailable(UNICODE) } == 0 {
        return Ok(None);
    }
    let _lock = Lock::open()?;
    // SAFETY: clipboard is open on this thread; the returned handle is borrowed for the duration of Lock.
    let handle = unsafe { GetClipboardData(UNICODE) };
    if handle.is_null() {
        return Ok(None);
    }
    // SAFETY: handle is a valid global memory object returned by GetClipboardData for CF_UNICODETEXT.
    let bytes = unsafe { GlobalSize(handle) };
    if bytes < 2 {
        return Ok(None);
    }
    // SAFETY: handle is valid while clipboard remains open; paired with GlobalUnlock below.
    let pointer = unsafe { GlobalLock(handle) };
    if pointer.is_null() {
        return Err("native clipboard lock failed".into());
    }
    let count = (bytes / 2).min(LIMIT);
    // SAFETY: GlobalLock returned a readable buffer of at least count UTF-16 code units until GlobalUnlock.
    let units = unsafe { std::slice::from_raw_parts(pointer.cast::<u16>(), count) };
    let text = decode(units);
    // SAFETY: handle was locked by GlobalLock above while clipboard remains open.
    unsafe {
        GlobalUnlock(handle);
    }
    Ok(Some(text))
}

pub(super) fn write(text: &str) -> Result<(), String> {
    let _guard = GUARD.lock().expect("clipboard lock");
    let units = encode(text);
    if units.len() > LIMIT {
        return Err("clipboard text exceeds budget".into());
    }
    let bytes = units
        .len()
        .checked_mul(2)
        .ok_or_else(|| "clipboard byte size overflow".to_string())?;
    let block = Block::allocate(bytes)?;
    // SAFETY: block.raw() is a freshly allocated GMEM_MOVEABLE handle of `bytes` bytes; paired with GlobalUnlock.
    let pointer = unsafe { GlobalLock(block.raw()) };
    if pointer.is_null() {
        return Err("native clipboard lock failed".into());
    }
    // SAFETY: pointer is writable for units.len() u16 code units and does not overlap units.
    unsafe {
        std::ptr::copy_nonoverlapping(units.as_ptr(), pointer.cast::<u16>(), units.len());
        GlobalUnlock(block.raw());
    }
    let _lock = Lock::open()?;
    // SAFETY: clipboard is open on this thread.
    if unsafe { EmptyClipboard() } == 0 {
        return Err("native clipboard clear failed".into());
    }
    // SAFETY: block.raw() is an unlocked GMEM_MOVEABLE CF_UNICODETEXT buffer; non-null return transfers ownership to Windows.
    let placed = unsafe { SetClipboardData(UNICODE, block.raw()) };
    if placed.is_null() {
        return Err("native clipboard write failed".into());
    }
    block.release();
    Ok(())
}

pub(super) fn encode(text: &str) -> Vec<u16> {
    let mut units = Vec::with_capacity(text.len() + 1);
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\0' => {}
            '\r' => {
                characters.next_if_eq(&'\n');
                units.push(b'\r' as u16);
                units.push(b'\n' as u16);
            }
            '\n' => {
                units.push(b'\r' as u16);
                units.push(b'\n' as u16);
            }
            _ => {
                let mut buffer = [0u16; 2];
                units.extend_from_slice(character.encode_utf16(&mut buffer));
            }
        }
    }
    units.push(0);
    units
}

pub(super) fn decode(units: &[u16]) -> String {
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(units.len());
    let raw = String::from_utf16_lossy(&units[..end]);
    let mut text = String::with_capacity(raw.len());
    let mut characters = raw.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\r' {
            characters.next_if_eq(&'\n');
            text.push('\n');
        } else {
            text.push(character);
        }
    }
    text
}

#[link(name = "user32")]
unsafe extern "system" {
    fn OpenClipboard(window: HWND) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn IsClipboardFormatAvailable(format: u32) -> i32;
    fn GetClipboardData(format: u32) -> *mut c_void;
    fn SetClipboardData(format: u32, memory: *mut c_void) -> *mut c_void;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    fn GlobalLock(memory: *mut c_void) -> *mut c_void;
    fn GlobalUnlock(memory: *mut c_void) -> i32;
    fn GlobalFree(memory: *mut c_void) -> *mut c_void;
    fn GlobalSize(memory: *mut c_void) -> usize;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codec() {
        let encoded = encode("hello\n中文🙂\r\nend\r\0tail");
        assert_eq!(encoded.last(), Some(&0));
        assert_eq!(decode(&encoded), "hello\n中文🙂\nend\ntail");
        assert_eq!(decode(&[b'a' as u16, 0, b'b' as u16]), "a");
        assert_eq!(encode(""), vec![0]);
        assert_eq!(decode(&[0]), "");
    }

    #[test]
    fn system() {
        let prior = match read() {
            Ok(value) => value,
            Err(_) => return,
        };
        let sample = "plectron\n中文🙂";
        if write(sample).is_ok() {
            assert_eq!(read().unwrap().as_deref(), Some(sample));
        }
        if let Some(previous) = prior {
            let _ = write(&previous);
        }
    }
}
