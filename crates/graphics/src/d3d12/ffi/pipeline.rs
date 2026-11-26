use super::types::Sample;
use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Constants {
    pub register: u32,
    pub space: u32,
    pub count: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Descriptor {
    pub register: u32,
    pub space: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Segment {
    pub kind: u32,
    pub count: u32,
    pub register: u32,
    pub space: u32,
    pub offset: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Table {
    pub count: u32,
    pub ranges: *const Segment,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Payload {
    pub table: Table,
    pub constants: Constants,
    pub descriptor: Descriptor,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Parameter {
    pub kind: u32,
    pub data: Payload,
    pub visibility: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Signature {
    pub count: u32,
    pub params: *const Parameter,
    pub samplers: u32,
    pub statics: *const c_void,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytecode {
    pub ptr: *const c_void,
    pub len: usize,
}

impl Default for Bytecode {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Attachment {
    pub enable: i32,
    pub logic: i32,
    pub source: u32,
    pub dest: u32,
    pub op: u32,
    pub srcalpha: u32,
    pub dstalpha: u32,
    pub opalpha: u32,
    pub logicop: u32,
    pub mask: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Blend {
    pub coverage: i32,
    pub independent: i32,
    pub targets: [Attachment; 8],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Raster {
    pub fill: u32,
    pub cull: u32,
    pub ccw: i32,
    pub bias: i32,
    pub clamp: f32,
    pub slope: f32,
    pub clip: i32,
    pub multisample: i32,
    pub lines: i32,
    pub forced: u32,
    pub conservative: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Stencil {
    pub fail: u32,
    pub depthfail: u32,
    pub pass: u32,
    pub func: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Depth {
    pub depth: i32,
    pub mask: u32,
    pub func: u32,
    pub stencil: i32,
    pub read: u8,
    pub write: u8,
    pub front: Stencil,
    pub back: Stencil,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Layout {
    pub elements: *const c_void,
    pub count: u32,
}

impl Default for Layout {
    fn default() -> Self {
        Self {
            elements: std::ptr::null(),
            count: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Stream {
    pub entries: *const c_void,
    pub count: u32,
    pub strides: *const u32,
    pub countstrides: u32,
    pub stream: u32,
}

impl Default for Stream {
    fn default() -> Self {
        Self {
            entries: std::ptr::null(),
            count: 0,
            strides: std::ptr::null(),
            countstrides: 0,
            stream: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Cache {
    pub ptr: *const c_void,
    pub len: usize,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            ptr: std::ptr::null(),
            len: 0,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pipeline {
    pub root: *mut c_void,
    pub vs: Bytecode,
    pub ps: Bytecode,
    pub ds: Bytecode,
    pub hs: Bytecode,
    pub gs: Bytecode,
    pub stream: Stream,
    pub blend: Blend,
    pub mask: u32,
    pub raster: Raster,
    pub depth: Depth,
    pub input: Layout,
    pub cut: u32,
    pub topology: u32,
    pub targets: u32,
    pub formats: [u32; 8],
    pub dsv: u32,
    pub sample: Sample,
    pub node: u32,
    pub cached: Cache,
    pub flags: u32,
}
