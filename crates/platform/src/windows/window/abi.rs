use super::{Error, error};
use crate::shared::{Filter, Mode};
use std::ffi::c_void;
use widestring::U16CString;
use windows_sys::{
    Win32::Foundation::HWND,
    core::{GUID, HRESULT},
};

pub(super) const OPEN: GUID = GUID::from_u128(0xdc1c5a9c_e88a_4dde_a5a1_60f82a20aef7);
pub(super) const SAVE: GUID = GUID::from_u128(0xc0b4e2f3_ba21_4773_8dba_335ec946eb8b);
pub(super) const DIALOG: GUID = GUID::from_u128(0x42f85136_db7e_439c_85f1_e4075d135fc8);
pub(super) const MULTI: GUID = GUID::from_u128(0xd57c7288_d4ad_4768_be02_9d969532d960);
pub(super) const SHELL: GUID = GUID::from_u128(0x43826d1e_e718_42ee_bc55_a1e261c37bfe);
pub(super) const FOLDERS: u32 = 0x0000_0020;
pub(super) const FILESYSTEM: u32 = 0x0000_0040;
pub(super) const MULTIPLE: u32 = 0x0000_0200;
pub(super) const PATH: u32 = 0x0000_0800;
pub(super) const EXISTS: u32 = 0x0000_1000;
pub(super) const DISPLAY: u32 = 0x8005_8000;
pub(super) const CANCELED: HRESULT = 0x8007_04C7u32 as i32;

pub(super) type Parse =
    unsafe extern "system" fn(*const u16, *mut c_void, *const GUID, *mut *mut c_void) -> HRESULT;

#[repr(C)]
pub(super) struct Spec {
    pub(super) name: *const u16,
    pub(super) pattern: *const u16,
}

#[repr(C)]
pub(super) struct Unknown {
    query: usize,
    add: usize,
    pub(super) release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
pub(super) struct File {
    unknown: [usize; 2],
    pub(super) release: unsafe extern "system" fn(*mut c_void) -> u32,
    pub(super) show: unsafe extern "system" fn(*mut c_void, HWND) -> HRESULT,
    pub(super) types: unsafe extern "system" fn(*mut c_void, u32, *const Spec) -> HRESULT,
    skip: [usize; 4],
    pub(super) options: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT,
    pub(super) current: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    default: usize,
    pub(super) folder: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    middle: [usize; 2],
    pub(super) name: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    read: usize,
    pub(super) title: unsafe extern "system" fn(*mut c_void, *const u16) -> HRESULT,
    labels: [usize; 2],
    pub(super) result: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
    tail: [usize; 6],
    pub(super) results: unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub(super) struct Item {
    unknown: [usize; 2],
    pub(super) release: unsafe extern "system" fn(*mut c_void) -> u32,
    skip: [usize; 2],
    pub(super) display: unsafe extern "system" fn(*mut c_void, u32, *mut *mut u16) -> HRESULT,
}

#[repr(C)]
pub(super) struct Items {
    unknown: [usize; 2],
    pub(super) release: unsafe extern "system" fn(*mut c_void) -> u32,
    skip: [usize; 4],
    pub(super) count: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT,
    pub(super) item: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> HRESULT,
}

pub(super) fn flags(current: u32, mode: Mode, multiple: bool) -> u32 {
    let base = current | FILESYSTEM | PATH;
    match mode {
        Mode::Folder => (base | FOLDERS) & !MULTIPLE,
        Mode::Save => base & !(FOLDERS | MULTIPLE),
        Mode::Open if multiple => (base | EXISTS | MULTIPLE) & !FOLDERS,
        Mode::Open => (base | EXISTS) & !(FOLDERS | MULTIPLE),
    }
}

pub(super) fn canceled(status: HRESULT) -> bool {
    status == CANCELED
}

pub(super) fn check(status: HRESULT, name: &str) -> Result<(), Error> {
    if status < 0 {
        return Err(Error(format!("{name} failed: 0x{:08x}", status as u32)));
    }
    Ok(())
}

pub(super) fn specs(filters: &[Filter]) -> Result<Vec<(U16CString, U16CString)>, Error> {
    let mut pairs = Vec::with_capacity(filters.len());
    for filter in filters {
        let label = U16CString::from_str(&filter.label).map_err(error)?;
        let spec = pattern(&filter.extensions);
        let wide = U16CString::from_str(&spec).map_err(error)?;
        pairs.push((label, wide));
    }
    Ok(pairs)
}

pub(super) fn pattern(extensions: &[String]) -> String {
    let parts: Vec<String> = extensions
        .iter()
        .map(|raw| raw.trim().trim_start_matches('.'))
        .filter(|clean| !clean.is_empty())
        .map(|clean| format!("*.{clean}"))
        .collect();
    if parts.is_empty() {
        "*.*".into()
    } else {
        parts.join(";")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::{align_of, offset_of, size_of};

    #[test]
    fn layout() {
        let word = size_of::<usize>();
        assert_eq!(size_of::<Spec>(), 2 * word);
        assert_eq!(align_of::<Spec>(), word);
        assert_eq!(offset_of!(File, release), 2 * word);
        assert_eq!(offset_of!(File, show), 3 * word);
        assert_eq!(offset_of!(File, types), 4 * word);
        assert_eq!(offset_of!(File, options), 9 * word);
        assert_eq!(offset_of!(File, current), 10 * word);
        assert_eq!(offset_of!(File, folder), 12 * word);
        assert_eq!(offset_of!(File, name), 15 * word);
        assert_eq!(offset_of!(File, title), 17 * word);
        assert_eq!(offset_of!(File, result), 20 * word);
        assert_eq!(offset_of!(File, results), 27 * word);
        assert_eq!(offset_of!(Item, release), 2 * word);
        assert_eq!(offset_of!(Item, display), 5 * word);
        assert_eq!(offset_of!(Items, release), 2 * word);
        assert_eq!(offset_of!(Items, count), 7 * word);
        assert_eq!(offset_of!(Items, item), 8 * word);
    }

    #[test]
    fn encoding() {
        assert_eq!(pattern(&[]), "*.*");
        assert_eq!(
            pattern(&["txt".into(), ".md".into(), "  ".into()]),
            "*.txt;*.md"
        );
        assert!(canceled(0x8007_04C7u32 as i32));
        assert!(!canceled(0));
        let pick = flags(0, Mode::Folder, true);
        assert_ne!(pick & FOLDERS, 0);
        assert_ne!(pick & FILESYSTEM, 0);
        assert_eq!(pick & MULTIPLE, 0);
        let multi = flags(0, Mode::Open, true);
        assert_ne!(multi & MULTIPLE, 0);
        assert_ne!(multi & EXISTS, 0);
        assert_eq!(multi & FOLDERS, 0);
        let save = flags(MULTIPLE, Mode::Save, true);
        assert_eq!(save & MULTIPLE, 0);
        assert_eq!(save & FOLDERS, 0);
    }
}
