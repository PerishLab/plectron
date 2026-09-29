use std::ffi::c_void;
use std::ptr::NonNull;

const PAGE: usize = 16384;
const READ: i32 = 1;
const WRITE: i32 = 2;
const EXECUTE: i32 = 4;
const PRIVATE: i32 = 2;
const ANONYMOUS: i32 = 0x1000;
const JIT: i32 = 0x800;

pub(super) struct Memory {
    pointer: NonNull<c_void>,
}

// SAFETY: ownership transfers an immutable mapping; execution and destruction borrow its owner.
unsafe impl Send for Memory {}

impl Memory {
    pub fn new(words: &[u32]) -> Result<Self, String> {
        unsafe extern "C" {
            fn mmap(
                p: *mut c_void,
                len: usize,
                prot: i32,
                flags: i32,
                fd: i32,
                off: i64,
            ) -> *mut c_void;
            #[link_name = "pthread_jit_write_protect_np"]
            fn protect(enabled: i32);
            #[link_name = "sys_icache_invalidate"]
            fn invalidate(p: *mut c_void, len: usize);
        }

        let bytes = std::mem::size_of_val(words);
        if bytes > PAGE || words.is_empty() {
            return Err("native code page budget exceeded".into());
        }
        // SAFETY: anonymous private MAP_JIT mapping; failure is checked before access.
        let pointer = unsafe {
            mmap(
                std::ptr::null_mut(),
                PAGE,
                READ | WRITE | EXECUTE,
                PRIVATE | ANONYMOUS | JIT,
                -1,
                0,
            )
        };
        if pointer as isize == -1 {
            return Err(format!(
                "executable allocation: {}",
                std::io::Error::last_os_error()
            ));
        }
        let pointer = NonNull::new(pointer).expect("nonzero anonymous mapping");
        // SAFETY: bounded copy into exclusively owned storage; no callbacks or panics while writable.
        unsafe {
            protect(0);
            std::ptr::copy_nonoverlapping(words.as_ptr(), pointer.as_ptr().cast(), words.len());
            protect(1);
            invalidate(pointer.as_ptr(), bytes);
        }
        Ok(Self { pointer })
    }

    pub fn run(&self, arguments: &[f64]) -> f64 {
        let mut scratch = [0.0; 256];
        // SAFETY: only the validated private emitter creates this leaf ABI. It reads bounded
        // arguments, writes at most 256 scratch slots, preserves callee registers and returns d0.
        unsafe {
            let function: unsafe extern "C" fn(*const f64, *mut f64) -> f64 =
                std::mem::transmute(self.pointer.as_ptr());
            function(arguments.as_ptr(), scratch.as_mut_ptr())
        }
    }
}

impl Drop for Memory {
    fn drop(&mut self) {
        unsafe extern "C" {
            fn munmap(p: *mut c_void, len: usize) -> i32;
        }
        // SAFETY: this is the uniquely owned mapping; no executable pointer escapes its lifetime.
        unsafe { munmap(self.pointer.as_ptr(), PAGE) };
    }
}
