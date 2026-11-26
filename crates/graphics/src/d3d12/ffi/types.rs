use std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Queue {
    pub kind: u32,
    pub priority: i32,
    pub flags: u32,
    pub mask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Heap {
    pub kind: u32,
    pub count: u32,
    pub flags: u32,
    pub mask: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Cpu {
    pub ptr: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Gpu {
    pub ptr: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Props {
    pub kind: u32,
    pub page: u32,
    pub pool: u32,
    pub creation: u32,
    pub visible: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Sample {
    pub count: u32,
    pub quality: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Resource {
    pub dimension: u32,
    pub alignment: u64,
    pub width: u64,
    pub height: u32,
    pub depth: u16,
    pub mips: u16,
    pub format: u32,
    pub sample: Sample,
    pub layout: u32,
    pub flags: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Allocation {
    pub bytes: u64,
    pub alignment: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Clear {
    pub format: u32,
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Range {
    pub begin: usize,
    pub end: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Transition {
    pub resource: *mut c_void,
    pub subresource: u32,
    pub before: u32,
    pub after: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Barrier {
    pub kind: u32,
    pub flags: u32,
    pub transition: Transition,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Footprint {
    pub format: u32,
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub pitch: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Placed {
    pub offset: u64,
    pub footprint: Footprint,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union Copy {
    pub placed: Placed,
    pub index: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Location {
    pub resource: *mut c_void,
    pub kind: u32,
    pub data: Copy,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub near: f32,
    pub far: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Scissor {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Texture {
    pub detailed: u32,
    pub levels: u32,
    pub plane: u32,
    pub clamp: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union View {
    pub texture: Texture,
    pub buffer: [u64; 3],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Shader {
    pub format: u32,
    pub dimension: u32,
    pub mapping: u32,
    pub data: View,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Target {
    pub format: u32,
    pub dimension: u32,
    pub mip: u32,
    pub plane: u32,
    pub pad: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Query {
    pub kind: u32,
    pub count: u32,
    pub mask: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Adapter {
    pub name: [u16; 128],
    pub vendor: u32,
    pub device: u32,
    pub subsys: u32,
    pub revision: u32,
    pub vram: usize,
    pub sysram: usize,
    pub shared: usize,
    pub luidlow: u32,
    pub luidhigh: i32,
    pub flags: u32,
}

impl Default for Adapter {
    fn default() -> Self {
        Self {
            name: [0; 128],
            vendor: 0,
            device: 0,
            subsys: 0,
            revision: 0,
            vram: 0,
            sysram: 0,
            shared: 0,
            luidlow: 0,
            luidhigh: 0,
            flags: 0,
        }
    }
}
