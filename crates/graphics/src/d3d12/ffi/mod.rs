pub mod adapter;
pub mod constants;
pub mod pipeline;
pub mod tables;
pub mod types;

use crate::shared::Fault;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::OnceLock;
use std::time::Duration;

pub type Hresult = i32;
pub type Handle = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    pub first: u32,
    pub second: u16,
    pub third: u16,
    pub tail: [u8; 8],
}

impl Guid {
    pub const fn new(first: u32, second: u16, third: u16, tail: [u8; 8]) -> Self {
        Self {
            first,
            second,
            third,
            tail,
        }
    }
}

#[repr(transparent)]
#[derive(Debug)]
pub struct Com(NonNull<c_void>);

impl Com {
    pub unsafe fn own(pointer: *mut c_void) -> Result<Self, Fault> {
        NonNull::new(pointer)
            .map(Self)
            .ok_or_else(|| Fault("D3D12 returned null COM object".into()))
    }

    pub fn raw(&self) -> *mut c_void {
        self.0.as_ptr()
    }

    pub unsafe fn vtable<T>(&self) -> &T {
        // SAFETY: Every COM object starts with an immutable vtable matching T.
        unsafe { &**(self.0.as_ptr() as *const *const T) }
    }

    pub fn query(&self, iid: &Guid) -> Option<Com> {
        let mut out = std::ptr::null_mut();
        // SAFETY: QueryInterface occupies slot 0 of every COM vtable.
        let hr = unsafe {
            let table = self.vtable::<tables::Unknown>();
            (table.query)(self.raw(), iid, &mut out)
        };
        if hr < 0 {
            return None;
        }
        // SAFETY: QueryInterface succeeded and transferred ownership into out.
        unsafe { Com::own(out).ok() }
    }
}

impl Clone for Com {
    fn clone(&self) -> Self {
        // SAFETY: AddRef occupies slot 1 of every COM vtable.
        unsafe {
            let table = self.vtable::<tables::Unknown>();
            (table.add)(self.raw());
        }
        Self(self.0)
    }
}

impl PartialEq for Com {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Com {}

impl Drop for Com {
    fn drop(&mut self) {
        // SAFETY: Release occupies slot 2 of every COM vtable.
        unsafe {
            let table = self.vtable::<tables::Unknown>();
            (table.release)(self.raw());
        }
    }
}

pub fn check(hr: Hresult, context: &str) -> Result<(), Fault> {
    if hr >= 0 {
        Ok(())
    } else {
        Err(Fault(format!(
            "{context} failed (HRESULT 0x{:08x})",
            hr as u32
        )))
    }
}

pub struct Event(Handle);

impl Event {
    pub fn new() -> Result<Self, Fault> {
        // SAFETY: Unnamed auto-reset Win32 event requires no external pointers.
        let handle = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
        if handle.is_null() {
            return Err(Fault("Win32 fence event allocation failed".into()));
        }
        Ok(Self(handle))
    }

    pub fn raw(&self) -> Handle {
        self.0
    }

    pub fn wait(&self, timeout: Duration) -> Result<bool, Fault> {
        let millis = timeout.as_millis().min(u32::MAX as u128 - 1) as u32;
        // SAFETY: self.0 is a live Win32 event handle owned by this wrapper.
        match unsafe { WaitForSingleObject(self.0, millis) } {
            0 => Ok(true),
            0x0000_0102 => Ok(false),
            code => Err(Fault(format!("Win32 fence wait failed (0x{code:08x})"))),
        }
    }
}

impl Drop for Event {
    fn drop(&mut self) {
        // SAFETY: self.0 is exclusively owned by this Event.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

pub type Device =
    unsafe extern "system" fn(*mut c_void, u32, *const Guid, *mut *mut c_void) -> Hresult;
pub type Serialize = unsafe extern "system" fn(
    *const pipeline::Signature,
    u32,
    *mut *mut c_void,
    *mut *mut c_void,
) -> Hresult;
pub type Factory = unsafe extern "system" fn(u32, *const Guid, *mut *mut c_void) -> Hresult;

pub struct Loader {
    pub device: Device,
    pub serialize: Serialize,
    pub factory: Factory,
}

static LOADER: OnceLock<Result<Loader, String>> = OnceLock::new();

pub fn loader() -> Result<&'static Loader, Fault> {
    LOADER
        .get_or_init(load)
        .as_ref()
        .map_err(|message| Fault(message.clone()))
}

fn load() -> Result<Loader, String> {
    let d3d12 = library(&utf16("d3d12.dll"))?;
    let dxgi = library(&utf16("dxgi.dll"))?;
    let device = symbol(d3d12, c"D3D12CreateDevice")?;
    let serialize = symbol(d3d12, c"D3D12SerializeRootSignature")?;
    let factory = symbol(dxgi, c"CreateDXGIFactory2")?;
    // SAFETY: Each resolved export matches the documented Win32 D3D12/DXGI ABI.
    unsafe {
        Ok(Loader {
            device: std::mem::transmute::<*const c_void, Device>(device),
            serialize: std::mem::transmute::<*const c_void, Serialize>(serialize),
            factory: std::mem::transmute::<*const c_void, Factory>(factory),
        })
    }
}

#[cfg(test)]
pub fn loaded(name: &str) -> bool {
    let wide = utf16(name);
    // SAFETY: wide is a NUL-terminated UTF-16 DLL name; GetModuleHandleW does not increment refcounts.
    unsafe { !GetModuleHandleW(wide.as_ptr()).is_null() }
}

fn utf16(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn library(name: &[u16]) -> Result<Handle, String> {
    // SAFETY: name is a NUL-terminated UTF-16 DLL name; system modules stay resident.
    let module = unsafe { LoadLibraryW(name.as_ptr()) };
    if module.is_null() {
        return Err("required Direct3D system library unavailable".into());
    }
    Ok(module)
}

fn symbol(module: Handle, name: &std::ffi::CStr) -> Result<*const c_void, String> {
    // SAFETY: module is a live HMODULE and name is a NUL-terminated C export name.
    let proc = unsafe { GetProcAddress(module, name.as_ptr()) };
    if proc.is_null() {
        return Err("required Direct3D export unavailable".into());
    }
    Ok(proc)
}

pub fn qpc() -> Option<(u64, u64)> {
    let mut counter = 0i64;
    let mut frequency = 0i64;
    // SAFETY: counter and frequency are valid writable i64 outputs for Win32 QPC queries.
    let ok = unsafe {
        QueryPerformanceCounter(&mut counter) != 0 && QueryPerformanceFrequency(&mut frequency) != 0
    };
    (ok && counter >= 0 && frequency > 0).then_some((counter as u64, frequency as u64))
}

type Composition = unsafe extern "system" fn(Handle, *mut Timing) -> Hresult;

#[repr(C, packed(1))]
struct Timing {
    size: u32,
    refresh: [u32; 2],
    period: u64,
    compose: [u32; 2],
    vblank: u64,
    count: u64,
    tail: [u8; 248],
}

static DWM: OnceLock<Option<Composition>> = OnceLock::new();

pub fn dwm() -> Option<(u64, u64)> {
    let query = (*DWM.get_or_init(|| {
        let module = library(&utf16("dwmapi.dll")).ok()?;
        let proc = symbol(module, c"DwmGetCompositionTimingInfo").ok()?;
        // SAFETY: DwmGetCompositionTimingInfo matches the documented dwmapi.dll ABI.
        Some(unsafe { std::mem::transmute::<*const c_void, Composition>(proc) })
    }))?;
    let mut info = Timing {
        size: std::mem::size_of::<Timing>() as u32,
        refresh: [0; 2],
        period: 0,
        compose: [0; 2],
        vblank: 0,
        count: 0,
        tail: [0; 248],
    };
    // SAFETY: null HWND queries desktop composition timing on Windows 8+; info has exact cbSize = 292.
    let hr = unsafe { query(std::ptr::null_mut(), &mut info) };
    if hr < 0 {
        return None;
    }
    let count = info.count;
    let vblank = info.vblank;
    Some((count, vblank))
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn LoadLibraryW(name: *const u16) -> Handle;
    #[cfg(test)]
    fn GetModuleHandleW(name: *const u16) -> Handle;
    fn GetProcAddress(module: Handle, name: *const i8) -> *const c_void;
    fn CreateEventW(security: *const c_void, manual: i32, initial: i32, name: *const u16)
    -> Handle;
    fn WaitForSingleObject(handle: Handle, millis: u32) -> u32;
    fn CloseHandle(handle: Handle) -> i32;
    fn QueryPerformanceCounter(value: *mut i64) -> i32;
    fn QueryPerformanceFrequency(value: *mut i64) -> i32;
}
