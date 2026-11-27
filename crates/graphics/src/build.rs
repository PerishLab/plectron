fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=resources/draw.hlsl");
    compiler::build();
}

#[cfg(not(windows))]
mod compiler {
    pub fn build() {
        panic!("precompiling Windows DXBC shaders requires a Windows host with d3dcompiler_47.dll");
    }
}

#[cfg(windows)]
mod compiler {
    use std::ffi::{CStr, c_void};
    use std::path::PathBuf;

    const MAGIC: &[u8; 4] = b"PDXB";
    const VERSION: u32 = 1;
    const OPTIMIZE: u32 = (1 << 15) | (1 << 13);
    const ENTRIES: [(&CStr, &CStr); 9] = [
        (c"vertexMain", c"vs_5_0"),
        (c"vertexPath", c"vs_5_0"),
        (c"vertexCoverage", c"vs_5_0"),
        (c"fragmentMain", c"ps_5_0"),
        (c"fragmentComposite", c"ps_5_0"),
        (c"fragmentPath", c"ps_5_0"),
        (c"fragmentCurve", c"ps_5_0"),
        (c"fragmentImage", c"ps_5_0"),
        (c"fragmentCoverage", c"ps_5_0"),
    ];

    type Handle = *mut c_void;
    type Hresult = i32;
    type Compile = unsafe extern "system" fn(
        *const c_void,
        usize,
        *const i8,
        *const c_void,
        *const c_void,
        *const i8,
        *const i8,
        u32,
        u32,
        *mut *mut c_void,
        *mut *mut c_void,
    ) -> Hresult;

    #[repr(C)]
    struct Unknown {
        query: *const c_void,
        add: *const c_void,
        release: unsafe extern "system" fn(*mut c_void) -> u32,
    }

    #[repr(C)]
    struct Blob {
        base: Unknown,
        pointer: unsafe extern "system" fn(*mut c_void) -> *const c_void,
        size: unsafe extern "system" fn(*mut c_void) -> usize,
    }

    pub fn build() {
        let source = std::fs::read("resources/draw.hlsl").expect("read draw.hlsl");
        let api = load();
        let mut blobs = Vec::with_capacity(ENTRIES.len());
        for (entry, profile) in ENTRIES {
            blobs.push(shader(api, &source, entry, profile));
        }
        let table = encode(&blobs);
        let dir = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR"));
        std::fs::write(dir.join("draw.dxbc"), table).expect("write draw.dxbc");
    }

    fn load() -> Compile {
        // SAFETY: c"d3dcompiler_47.dll" is a static NUL-terminated ASCII DLL name.
        let module = unsafe { LoadLibraryA(c"d3dcompiler_47.dll".as_ptr()) };
        if module.is_null() {
            panic!("failed to load d3dcompiler_47.dll for DXBC precompilation");
        }
        // SAFETY: module is a live HMODULE and c"D3DCompile" is a static NUL-terminated export name.
        let proc = unsafe { GetProcAddress(module, c"D3DCompile".as_ptr()) };
        if proc.is_null() {
            panic!("failed to resolve D3DCompile from d3dcompiler_47.dll");
        }
        // SAFETY: D3DCompile matches the documented Win32 FXC ABI.
        unsafe { std::mem::transmute::<*const c_void, Compile>(proc) }
    }

    fn shader(api: Compile, source: &[u8], entry: &CStr, profile: &CStr) -> Vec<u8> {
        let mut code = std::ptr::null_mut();
        let mut errors = std::ptr::null_mut();
        // SAFETY: source is live HLSL bytes; source name, entry and profile are static C strings.
        let hr = unsafe {
            api(
                source.as_ptr().cast(),
                source.len(),
                c"draw.hlsl".as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                entry.as_ptr(),
                profile.as_ptr(),
                OPTIMIZE,
                0,
                &mut code,
                &mut errors,
            )
        };
        if hr < 0 {
            let message = diagnostic(errors);
            panic!(
                "HLSL compile failed for {}: {message} (0x{:08x})",
                entry.to_string_lossy(),
                hr as u32
            );
        }
        if !errors.is_null() {
            release(errors);
        }
        if code.is_null() {
            panic!(
                "D3DCompile returned null bytecode for {}",
                entry.to_string_lossy()
            );
        }
        let data = bytes(code);
        release(code);
        data
    }

    fn diagnostic(raw: *mut c_void) -> String {
        if raw.is_null() {
            return "unknown compiler error".into();
        }
        let data = bytes(raw);
        release(raw);
        String::from_utf8_lossy(&data).trim().to_owned()
    }

    fn bytes(raw: *mut c_void) -> Vec<u8> {
        // SAFETY: raw is a live ID3D10Blob whose vtable starts with IUnknown followed by GetBufferPointer/Size.
        unsafe {
            let vtable = &**(raw as *const *const Blob);
            let ptr = (vtable.pointer)(raw).cast::<u8>();
            let len = (vtable.size)(raw);
            if ptr.is_null() || len == 0 {
                Vec::new()
            } else {
                std::slice::from_raw_parts(ptr, len).to_vec()
            }
        }
    }

    fn release(raw: *mut c_void) {
        // SAFETY: raw is an owned COM ID3D10Blob interface pointer.
        unsafe {
            let vtable = &**(raw as *const *const Blob);
            (vtable.base.release)(raw);
        }
    }

    fn encode(blobs: &[Vec<u8>]) -> Vec<u8> {
        let header = 12 + blobs.len() * 8;
        let payload: usize = blobs.iter().map(Vec::len).sum();
        let mut buffer = Vec::with_capacity(header + payload);
        buffer.extend_from_slice(MAGIC);
        buffer.extend_from_slice(&VERSION.to_le_bytes());
        buffer.extend_from_slice(&(blobs.len() as u32).to_le_bytes());
        let mut offset = header;
        for blob in blobs {
            let len = u32::try_from(blob.len()).expect("DXBC blob length");
            let pos = u32::try_from(offset).expect("DXBC table offset");
            buffer.extend_from_slice(&pos.to_le_bytes());
            buffer.extend_from_slice(&len.to_le_bytes());
            offset += blob.len();
        }
        for blob in blobs {
            buffer.extend_from_slice(blob);
        }
        buffer
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn LoadLibraryA(name: *const i8) -> Handle;
        fn GetProcAddress(module: Handle, name: *const i8) -> *const c_void;
    }
}
