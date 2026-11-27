use super::abi::{
    DIALOG, DISPLAY, File, Item, Items, MULTI, OPEN, Parse, SAVE, SHELL, Spec, Unknown, canceled,
    check, flags, specs,
};
use super::{Error, Window, error};
use crate::shared::{Dialog, Mode, Outcome};
use std::{
    ffi::{CStr, OsString, c_void},
    os::windows::ffi::{OsStrExt, OsStringExt},
    path::{Path, PathBuf},
    sync::OnceLock,
};
use widestring::U16CString;
use windows_sys::Win32::{
    Foundation::{HWND, RPC_E_CHANGED_MODE, S_FALSE, S_OK},
    System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoTaskMemFree, CoUninitialize,
        },
        LibraryLoader::{GetProcAddress, LoadLibraryW},
    },
};

static SHELL32: OnceLock<Option<Parse>> = OnceLock::new();

struct Apartment(bool);

impl Apartment {
    fn enter() -> Result<Self, Error> {
        // SAFETY: initializes STA COM on the caller thread; S_OK/S_FALSE are balanced in Drop.
        let status = unsafe { CoInitializeEx(std::ptr::null(), COINIT_APARTMENTTHREADED as u32) };
        if status == S_OK || status == S_FALSE {
            Ok(Self(true))
        } else if status == RPC_E_CHANGED_MODE {
            Ok(Self(false))
        } else {
            Err(Error(format!(
                "COM initialization failed: 0x{:08x}",
                status as u32
            )))
        }
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            // SAFETY: balances the S_OK/S_FALSE CoInitializeEx call made on this thread.
            unsafe { CoUninitialize() };
        }
    }
}

struct Com(*mut c_void);

impl Drop for Com {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 is an owned COM pointer whose vtable begins with IUnknown.
            unsafe {
                let table = &**(self.0 as *const *const Unknown);
                (table.release)(self.0);
            }
            self.0 = std::ptr::null_mut();
        }
    }
}

struct Buffer(*mut u16);

impl Drop for Buffer {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 was allocated by COM (IShellItem::GetDisplayName) via CoTaskMemAlloc.
            unsafe { CoTaskMemFree(self.0.cast()) };
            self.0 = std::ptr::null_mut();
        }
    }
}

impl Window {
    pub fn dialog(&self, request: &Dialog) -> Result<Outcome, Error> {
        let _awareness = self.check()?;
        run(self.handle, request)
    }
}

fn run(handle: HWND, request: &Dialog) -> Result<Outcome, Error> {
    let _apartment = Apartment::enter()?;
    let instance = create(request.mode)?;
    configure(&instance, request)?;
    // SAFETY: instance is a valid IFileDialog COM object and handle is a live owner-thread HWND.
    let table = unsafe { &**(instance.0 as *const *const File) };
    // SAFETY: Show runs outside WndProc with no State RefCell borrows held across the modal loop.
    let status = unsafe { (table.show)(instance.0, handle) };
    if canceled(status) {
        return Ok(Outcome::Canceled);
    }
    check(status, "IFileDialog::Show")?;
    collect(&instance, request)
}

fn create(mode: Mode) -> Result<Com, Error> {
    let (class, interface) = match mode {
        Mode::Open | Mode::Folder => (&OPEN, &MULTI),
        Mode::Save => (&SAVE, &DIALOG),
    };
    let mut raw = std::ptr::null_mut();
    // SAFETY: class and interface are valid Win32 COM GUIDs; raw receives an owned COM pointer on success.
    let status = unsafe {
        CoCreateInstance(
            class,
            std::ptr::null_mut(),
            CLSCTX_INPROC_SERVER,
            interface,
            &mut raw,
        )
    };
    check(status, "CoCreateInstance")?;
    if raw.is_null() {
        return Err(Error("COM dialog returned null instance".into()));
    }
    Ok(Com(raw))
}

fn configure(instance: &Com, request: &Dialog) -> Result<(), Error> {
    // SAFETY: instance is a live IFileDialog/IFileOpenDialog COM pointer.
    let table = unsafe { &**(instance.0 as *const *const File) };
    let mut current = 0u32;
    // SAFETY: current is writable and instance is a valid IFileDialog.
    check(
        unsafe { (table.current)(instance.0, &mut current) },
        "GetOptions",
    )?;
    let next = flags(current, request.mode, request.multiple);
    // SAFETY: next carries documented FOS_* flags on this live IFileDialog.
    check(unsafe { (table.options)(instance.0, next) }, "SetOptions")?;
    if let Some(title) = &request.title {
        let wide = U16CString::from_str(title).map_err(error)?;
        // SAFETY: wide is NUL-terminated UTF-16 valid for the synchronous SetTitle call.
        check(
            unsafe { (table.title)(instance.0, wide.as_ptr()) },
            "SetTitle",
        )?;
    }
    if let Some(name) = &request.name {
        let wide = U16CString::from_str(name).map_err(error)?;
        // SAFETY: wide is NUL-terminated UTF-16 valid for the synchronous SetFileName call.
        check(
            unsafe { (table.name)(instance.0, wide.as_ptr()) },
            "SetFileName",
        )?;
    }
    if let Some(directory) = &request.directory
        && let Some(item) = folder(directory)
    {
        // SAFETY: item is a live IShellItem owned for the duration of SetFolder.
        let _ = unsafe { (table.folder)(instance.0, item.0) };
    }
    if request.mode != Mode::Folder && !request.filters.is_empty() {
        let owned = specs(&request.filters)?;
        let raw: Vec<Spec> = owned
            .iter()
            .map(|(label, spec)| Spec {
                name: label.as_ptr(),
                pattern: spec.as_ptr(),
            })
            .collect();
        // SAFETY: owned keeps every UTF-16 label and pattern live throughout SetFileTypes.
        check(
            unsafe { (table.types)(instance.0, raw.len() as u32, raw.as_ptr()) },
            "SetFileTypes",
        )?;
    }
    Ok(())
}

fn collect(instance: &Com, request: &Dialog) -> Result<Outcome, Error> {
    // SAFETY: instance is a live IFileDialog/IFileOpenDialog after Show succeeded.
    let table = unsafe { &**(instance.0 as *const *const File) };
    if request.mode == Mode::Open && request.multiple {
        let mut array = std::ptr::null_mut();
        // SAFETY: Mode::Open created IFileOpenDialog so slot 27 GetResults is valid.
        check(
            unsafe { (table.results)(instance.0, &mut array) },
            "GetResults",
        )?;
        let list = Com(array);
        // SAFETY: list is a live IShellItemArray COM pointer.
        let vtable = unsafe { &**(list.0 as *const *const Items) };
        let mut total = 0u32;
        // SAFETY: total is writable and list is a valid IShellItemArray.
        check(unsafe { (vtable.count)(list.0, &mut total) }, "GetCount")?;
        let mut paths = Vec::with_capacity(total as usize);
        for index in 0..total {
            let mut raw = std::ptr::null_mut();
            // SAFETY: index < total on this live IShellItemArray; raw receives an owned IShellItem.
            check(
                unsafe { (vtable.item)(list.0, index, &mut raw) },
                "GetItemAt",
            )?;
            paths.push(decode(&Com(raw))?);
        }
        return Ok(Outcome::Selected(paths));
    }
    let mut raw = std::ptr::null_mut();
    // SAFETY: raw receives an owned IShellItem from IFileDialog::GetResult.
    check(unsafe { (table.result)(instance.0, &mut raw) }, "GetResult")?;
    Ok(Outcome::Selected(vec![decode(&Com(raw))?]))
}

fn decode(item: &Com) -> Result<PathBuf, Error> {
    // SAFETY: item is a live IShellItem COM pointer.
    let table = unsafe { &**(item.0 as *const *const Item) };
    let mut pointer = std::ptr::null_mut();
    // SAFETY: GetDisplayName allocates a NUL-terminated UTF-16 path freed by Buffer::drop.
    check(
        unsafe { (table.display)(item.0, DISPLAY, &mut pointer) },
        "GetDisplayName",
    )?;
    let buffer = Buffer(pointer);
    if buffer.0.is_null() {
        return Err(Error("IShellItem::GetDisplayName returned null".into()));
    }
    let mut length = 0usize;
    // SAFETY: Win32 guarantees buffer.0 is a NUL-terminated UTF-16 string.
    while unsafe { *buffer.0.add(length) } != 0 {
        length += 1;
    }
    // SAFETY: buffer.0 points to length initialized u16 code units.
    let slice = unsafe { std::slice::from_raw_parts(buffer.0, length) };
    Ok(PathBuf::from(OsString::from_wide(slice)))
}

fn folder(path: &Path) -> Option<Com> {
    let parse = (*SHELL32.get_or_init(loader))?;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return None;
    }
    wide.push(0);
    let mut raw = std::ptr::null_mut();
    // SAFETY: wide is NUL-terminated UTF-16 and SHELL is IID_IShellItem.
    let status = unsafe { parse(wide.as_ptr(), std::ptr::null_mut(), &SHELL, &mut raw) };
    (status >= 0 && !raw.is_null()).then_some(Com(raw))
}

fn loader() -> Option<Parse> {
    let name: Vec<u16> = "shell32.dll\0".encode_utf16().collect();
    // SAFETY: shell32.dll is a standard Windows system DLL that stays resident for process lifetime.
    let module = unsafe { LoadLibraryW(name.as_ptr()) };
    if module.is_null() {
        return None;
    }
    let export: &CStr = c"SHCreateItemFromParsingName";
    // SAFETY: module is a live HMODULE and export is a NUL-terminated ASCII symbol name.
    let address = unsafe { GetProcAddress(module, export.as_ptr().cast()) }?;
    // SAFETY: SHCreateItemFromParsingName matches the documented Parse extern "system" fn ABI.
    Some(unsafe { std::mem::transmute_copy(&address) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared::Filter;

    #[test]
    fn lifecycle() {
        let _apartment = Apartment::enter().unwrap();
        let mut open = Dialog::open();
        open.title = Some("Select Package".into());
        open.name = Some("app.pbnd".into());
        open.multiple = true;
        open.filters = vec![Filter {
            label: "Packages".into(),
            extensions: vec!["pbnd".into(), "zip".into()],
        }];
        configure(&create(open.mode).unwrap(), &open).unwrap();
        let mut save = Dialog::save();
        save.title = Some("Save Log".into());
        save.name = Some("install.log".into());
        configure(&create(save.mode).unwrap(), &save).unwrap();
        let mut pick = Dialog::folder();
        pick.title = Some("Install Directory".into());
        configure(&create(pick.mode).unwrap(), &pick).unwrap();
    }
}
