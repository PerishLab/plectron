use super::Error;
use crate::shared::fonts::{Asset, Discovery, Fonts as Contract, Tone};
use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use widestring::U16CString;
use windows_sys::Win32::Foundation::LPARAM;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateFontIndirectW, DEFAULT_CHARSET, DeleteDC, DeleteObject,
    EnumFontFamiliesExW, FW_BOLD, FW_NORMAL, GDI_ERROR, GetFontData, HDC, HFONT, LOGFONTW,
    SelectObject, TEXTMETRICW,
};

const COLLECTION: u32 = u32::from_le_bytes(*b"ttcf");
const FAILURE: u32 = GDI_ERROR as u32;
const DEFAULTS: &[&str] = &["Segoe UI", "Microsoft YaHei"];

#[derive(Default)]
pub struct Fonts {
    cache: Mutex<BTreeMap<(String, Tone), Option<Asset>>>,
}

impl Fonts {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Contract<Error> for Fonts {
    fn discover(&self, family: &str, tone: Tone) -> Result<Discovery, Error> {
        let name = alias(family);
        let key = (name.to_lowercase(), tone);
        if let Some(cached) = self.cache.lock().expect("font cache lock").get(&key) {
            return Ok(match cached {
                Some(asset) => Discovery::Found(asset.clone()),
                None => Discovery::Absent,
            });
        }
        let found = query(name, tone)?;
        let mut cache = self.cache.lock().expect("font cache lock");
        if cache.len() < 128 {
            cache.insert(key, found.clone());
        }
        Ok(match found {
            Some(asset) => Discovery::Found(asset),
            None => Discovery::Absent,
        })
    }

    fn defaults(&self) -> &'static [&'static str] {
        DEFAULTS
    }
}

fn alias(family: &str) -> &str {
    match family.to_ascii_lowercase().as_str() {
        "sans-serif" | "system-ui" => "Segoe UI",
        "monospace" => "Consolas",
        "serif" => "Times New Roman",
        _ => family,
    }
}

struct Context(HDC);
impl Drop for Context {
    fn drop(&mut self) {
        // SAFETY: self.0 is a memory device context uniquely owned by Context.
        unsafe {
            DeleteDC(self.0);
        }
    }
}

struct Handle(HFONT);
impl Drop for Handle {
    fn drop(&mut self) {
        // SAFETY: self.0 is a GDI font object uniquely owned by Handle and already unselected.
        unsafe {
            DeleteObject(self.0);
        }
    }
}

unsafe extern "system" fn enumerated(
    _: *const LOGFONTW,
    _: *const TEXTMETRICW,
    _: u32,
    state: LPARAM,
) -> i32 {
    // SAFETY: state points to the caller's live stack boolean during EnumFontFamiliesExW.
    unsafe {
        *(state as *mut bool) = true;
    }
    0
}

fn descriptor(family: &str, tone: Tone) -> Option<LOGFONTW> {
    let encoded = U16CString::from_str(family).ok()?;
    let slice = encoded.as_slice();
    if slice.is_empty() || slice.len() >= 32 {
        return None;
    }
    let mut font: LOGFONTW = unsafe { std::mem::zeroed() };
    font.lfWeight = match tone {
        Tone::Normal => FW_NORMAL as i32,
        Tone::Bold => FW_BOLD as i32,
    };
    font.lfCharSet = DEFAULT_CHARSET;
    font.lfFaceName[..slice.len()].copy_from_slice(slice);
    Some(font)
}

fn present(context: HDC, font: &mut LOGFONTW) -> bool {
    let mut matched = false;
    let state = (&mut matched as *mut bool) as LPARAM;
    // SAFETY: context is a valid memory DC, font is initialized, and state stays live until return.
    unsafe {
        EnumFontFamiliesExW(context, font, Some(enumerated), state, 0);
    }
    matched
}

fn read(context: HDC, table: u32) -> Result<Option<Vec<u8>>, Error> {
    // SAFETY: context is a valid DC with a selected font; null buffer queries exact byte size.
    let size = unsafe { GetFontData(context, table, 0, std::ptr::null_mut(), 0) };
    if size == FAILURE || size == 0 {
        return Ok(None);
    }
    let mut bytes = vec![0u8; size as usize];
    // SAFETY: bytes provides exactly size writable bytes for GetFontData.
    let copied =
        unsafe { GetFontData(context, table, 0, bytes.as_mut_ptr().cast::<c_void>(), size) };
    if copied == FAILURE || copied != size {
        return Err(Error("failed reading Windows system font bytes".into()));
    }
    Ok(Some(bytes))
}

fn query(family: &str, tone: Tone) -> Result<Option<Asset>, Error> {
    let Some(mut font) = descriptor(family, tone) else {
        return Ok(None);
    };
    // SAFETY: passing null creates a compatible memory DC for the current screen.
    let raw = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if raw.is_null() {
        return Err(Error("failed creating GDI font memory context".into()));
    }
    let context = Context(raw);
    if !present(context.0, &mut font) {
        return Ok(None);
    }
    // SAFETY: font is a valid initialized LOGFONTW.
    let created = unsafe { CreateFontIndirectW(&font) };
    if created.is_null() {
        return Err(Error("failed creating GDI logical font".into()));
    }
    let handle = Handle(created);
    // SAFETY: context.0 and handle.0 are valid GDI handles; previous object is restored before drop.
    let previous = unsafe { SelectObject(context.0, handle.0) };
    let result = extract(context.0, family, tone);
    // SAFETY: previous is the stock object returned by SelectObject on context.0.
    unsafe {
        SelectObject(context.0, previous);
    }
    result
}

fn extract(context: HDC, family: &str, tone: Tone) -> Result<Option<Asset>, Error> {
    // SAFETY: context has the resolved logical font selected; null buffer queries face size.
    let face = unsafe { GetFontData(context, 0, 0, std::ptr::null_mut(), 0) };
    if face == FAILURE || face == 0 {
        return Ok(None);
    }
    if let Some(collection) = read(context, COLLECTION)? {
        let offset = u32::try_from(collection.len())
            .ok()
            .and_then(|total| total.checked_sub(face))
            .ok_or_else(|| Error("invalid TTC face offset from Windows GDI".into()))?;
        let index = slot(&collection, offset).unwrap_or(0);
        return Ok(Some(Asset {
            family: family.into(),
            tone,
            bytes: Arc::from(collection),
            index,
        }));
    }
    let Some(bytes) = read(context, 0)? else {
        return Ok(None);
    };
    Ok(Some(Asset {
        family: family.into(),
        tone,
        bytes: Arc::from(bytes),
        index: 0,
    }))
}

fn slot(bytes: &[u8], offset: u32) -> Option<u32> {
    if bytes.len() < 12 || &bytes[0..4] != b"ttcf" {
        return None;
    }
    let count = u32::from_be_bytes(bytes[8..12].try_into().ok()?);
    for index in 0..count {
        let start = 12usize.checked_add(usize::try_from(index).ok()?.checked_mul(4)?)?;
        let entry = u32::from_be_bytes(bytes.get(start..start + 4)?.try_into().ok()?);
        if entry == offset {
            return Some(index);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host() {
        let fonts = Fonts::new();
        for tone in [Tone::Normal, Tone::Bold] {
            let Discovery::Found(latin) = fonts.discover("Segoe UI", tone).unwrap() else {
                panic!("expected Segoe UI on Windows");
            };
            assert_eq!(&latin.bytes[0..4], &0x0001_0000u32.to_be_bytes());
            assert_eq!(latin.index, 0);
            let Discovery::Found(chinese) = fonts.discover("Microsoft YaHei", tone).unwrap() else {
                panic!("expected Microsoft YaHei on Windows");
            };
            assert_eq!(&chinese.bytes[0..4], b"ttcf");
            assert_eq!(chinese.index, 0);
        }
        let Discovery::Found(interface) =
            fonts.discover("Microsoft YaHei UI", Tone::Normal).unwrap()
        else {
            panic!("expected Microsoft YaHei UI on Windows");
        };
        assert_eq!(&interface.bytes[0..4], b"ttcf");
        assert_eq!(interface.index, 1);
        assert!(matches!(
            fonts.discover("sans-serif", Tone::Normal).unwrap(),
            Discovery::Found(_)
        ));
        assert_eq!(
            fonts
                .discover("DefinitelyAbsentFont999", Tone::Normal)
                .unwrap(),
            Discovery::Absent
        );
    }
}
