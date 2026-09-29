use crate::Outcome;

#[cfg(target_os = "macos")]
pub fn begin(id: u64, session: u64, parent: Option<u64>, name: &str) -> bool {
    unsafe extern "C" {
        fn plectron_observe_begin(
            id: u64,
            session: u64,
            parent: u64,
            name: *const std::ffi::c_char,
        ) -> i32;
    }
    let Ok(name) = std::ffi::CString::new(name) else {
        return false;
    };
    // SAFETY: the bridge reads this live NUL-terminated string synchronously and retains no pointer.
    unsafe { plectron_observe_begin(id, session, parent.unwrap_or(0), name.as_ptr()) != 0 }
}
#[cfg(target_os = "macos")]
pub fn end(id: u64, outcome: Outcome) {
    unsafe extern "C" {
        fn plectron_observe_end(id: u64, outcome: *const std::ffi::c_char);
    }
    let name = match outcome {
        Outcome::Success => c"success",
        Outcome::Failure => c"failure",
        Outcome::Abandoned => c"abandoned",
        Outcome::Panicked => c"panicked",
    };
    // SAFETY: the matching begin initialized the logger; the outcome string is static.
    unsafe { plectron_observe_end(id, name.as_ptr()) }
}
#[cfg(target_os = "macos")]
pub fn cpu() -> Option<u64> {
    unsafe extern "C" {
        fn plectron_observe_cpu() -> u64;
    }
    // SAFETY: the bridge reads only the calling thread's clock and returns a value or sentinel.
    let value = unsafe { plectron_observe_cpu() };
    (value != u64::MAX).then_some(value)
}
#[cfg(target_os = "macos")]
pub fn thread() -> Option<u64> {
    unsafe extern "C" {
        fn plectron_observe_thread() -> u64;
    }
    // SAFETY: the bridge owns its output storage and returns the current native thread identifier.
    let value = unsafe { plectron_observe_thread() };
    (value != 0).then_some(value)
}
#[cfg(not(target_os = "macos"))]
pub fn begin(_: u64, _: u64, _: Option<u64>, _: &str) -> bool {
    false
}
#[cfg(not(target_os = "macos"))]
pub fn end(_: u64, _: Outcome) {}
#[cfg(not(target_os = "macos"))]
pub fn cpu() -> Option<u64> {
    None
}
#[cfg(not(target_os = "macos"))]
pub fn thread() -> Option<u64> {
    None
}
