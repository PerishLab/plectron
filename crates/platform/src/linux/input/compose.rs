use std::ffi::{CStr, c_char, c_int, c_void};
use std::ptr::NonNull;

const COMPOSING: c_int = 1;
const COMPOSED: c_int = 2;
const CANCELLED: c_int = 3;
const ACCEPTED: c_int = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Outcome {
    Pass,
    Hold,
    Text(String),
}

type Create = unsafe extern "C" fn(c_int) -> *mut c_void;
type Locale = unsafe extern "C" fn(*mut c_void, *const c_char, c_int) -> *mut c_void;
type Start = unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void;
type Release = unsafe extern "C" fn(*mut c_void);
type Feed = unsafe extern "C" fn(*mut c_void, u32) -> c_int;
type Status = unsafe extern "C" fn(*mut c_void) -> c_int;
type Utf8 = unsafe extern "C" fn(*mut c_void, *mut c_char, usize) -> c_int;

struct Symbols {
    unref: Release,
    tables: Release,
    states: Release,
    feed: Feed,
    reset: Release,
    status: Status,
    utf8: Utf8,
}

pub(super) struct Compose {
    symbols: Symbols,
    context: NonNull<c_void>,
    table: NonNull<c_void>,
    state: NonNull<c_void>,
}
impl std::fmt::Debug for Compose {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Compose")
    }
}
impl Compose {
    pub(super) fn load() -> Option<Self> {
        // SAFETY: dlopen with a constant soname; failure returns null and is handled; the handle is never closed so resolved symbols stay valid.
        let library = unsafe {
            libc::dlopen(
                c"libxkbcommon.so.0".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            )
        };
        let library = NonNull::new(library)?;
        // SAFETY: each symbol is the documented xkbcommon C ABI matching the declared function pointer type.
        let (create, locale, start) = unsafe {
            (
                lookup::<Create>(library, c"xkb_context_new")?,
                lookup::<Locale>(library, c"xkb_compose_table_new_from_locale")?,
                lookup::<Start>(library, c"xkb_compose_state_new")?,
            )
        };
        // SAFETY: same documented ABI as above.
        let symbols = unsafe {
            Symbols {
                unref: lookup(library, c"xkb_context_unref")?,
                tables: lookup(library, c"xkb_compose_table_unref")?,
                states: lookup(library, c"xkb_compose_state_unref")?,
                feed: lookup(library, c"xkb_compose_state_feed")?,
                reset: lookup(library, c"xkb_compose_state_reset")?,
                status: lookup(library, c"xkb_compose_state_get_status")?,
                utf8: lookup(library, c"xkb_compose_state_get_utf8")?,
            }
        };
        // SAFETY: xkb_context_new with no flags returns an owned context or null.
        let context = NonNull::new(unsafe { create(0) })?;
        // SAFETY: the context is live; "C" resolves to the UTF-8 compose table and honours XCOMPOSEFILE and ~/.XCompose.
        let table = NonNull::new(unsafe { locale(context.as_ptr(), c"C".as_ptr(), 0) });
        let Some(table) = table else {
            // SAFETY: the context is owned here and unused afterward.
            unsafe { (symbols.unref)(context.as_ptr()) };
            return None;
        };
        // SAFETY: the table is live; the state takes its own table reference.
        let state = NonNull::new(unsafe { start(table.as_ptr(), 0) });
        let Some(state) = state else {
            // SAFETY: table and context are owned here and unused afterward.
            unsafe {
                (symbols.tables)(table.as_ptr());
                (symbols.unref)(context.as_ptr());
            }
            return None;
        };
        Some(Self {
            symbols,
            context,
            table,
            state,
        })
    }
    pub(super) fn feed(&mut self, keysym: u32) -> Outcome {
        let state = self.state.as_ptr();
        // SAFETY: the state is owned by self and live until drop.
        if unsafe { (self.symbols.feed)(state, keysym) } != ACCEPTED {
            return Outcome::Pass;
        }
        // SAFETY: the state is owned by self and live until drop.
        match unsafe { (self.symbols.status)(state) } {
            COMPOSING => Outcome::Hold,
            CANCELLED => {
                self.reset();
                Outcome::Hold
            }
            COMPOSED => {
                let text = self.text();
                self.reset();
                text.map_or(Outcome::Hold, Outcome::Text)
            }
            _ => Outcome::Pass,
        }
    }
    pub(super) fn reset(&mut self) {
        // SAFETY: the state is owned by self and live until drop.
        unsafe { (self.symbols.reset)(self.state.as_ptr()) };
    }
    fn text(&self) -> Option<String> {
        let mut buffer = [0 as c_char; 64];
        // SAFETY: the buffer is writable for its full length; xkbcommon NUL-terminates within it.
        let length =
            unsafe { (self.symbols.utf8)(self.state.as_ptr(), buffer.as_mut_ptr(), buffer.len()) };
        if length <= 0 || length as usize >= buffer.len() {
            return None;
        }
        // SAFETY: a positive in-range length means the buffer holds a NUL-terminated string.
        let value = unsafe { CStr::from_ptr(buffer.as_ptr()) };
        value.to_str().ok().map(str::to_owned)
    }
}
impl Drop for Compose {
    fn drop(&mut self) {
        // SAFETY: each object is owned by self and released exactly once here.
        unsafe {
            (self.symbols.states)(self.state.as_ptr());
            (self.symbols.tables)(self.table.as_ptr());
            (self.symbols.unref)(self.context.as_ptr());
        }
    }
}

unsafe fn lookup<T: Copy>(library: NonNull<c_void>, name: &CStr) -> Option<T> {
    // SAFETY: the caller guarantees T is the function pointer type of the named symbol.
    let symbol = unsafe { libc::dlsym(library.as_ptr(), name.as_ptr()) };
    if symbol.is_null() {
        return None;
    }
    // SAFETY: T is a pointer-sized function pointer per the caller contract.
    Some(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&symbol) })
}
