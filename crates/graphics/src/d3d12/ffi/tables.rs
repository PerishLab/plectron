use super::{Guid, Handle, Hresult, pipeline, types};
use std::ffi::c_void;

type Slot = *const c_void;

#[repr(C)]
pub struct Unknown {
    pub query: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> Hresult,
    pub add: unsafe extern "system" fn(*mut c_void) -> u32,
    pub release: unsafe extern "system" fn(*mut c_void) -> u32,
}

#[repr(C)]
pub struct Blob {
    pub base: Unknown,
    pub pointer: unsafe extern "system" fn(*mut c_void) -> *const c_void,
    pub size: unsafe extern "system" fn(*mut c_void) -> usize,
}

#[repr(C)]
pub struct Adapter {
    pub base: Unknown,
    pub pad: [Slot; 7],
    pub desc: unsafe extern "system" fn(*mut c_void, *mut types::Adapter) -> Hresult,
}

#[repr(C)]
pub struct Factory4 {
    pub base: Unknown,
    pub pad0: [Slot; 9],
    pub adapters: unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> Hresult,
    pub pad1: [Slot; 14],
    pub warp: unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> Hresult,
}

#[repr(C)]
pub struct Factory6 {
    pub base: Factory4,
    pub check: Slot,
    pub prefer:
        unsafe extern "system" fn(*mut c_void, u32, u32, *const Guid, *mut *mut c_void) -> Hresult,
}

#[repr(C)]
pub struct Device {
    pub base: Unknown,
    pub pad0: [Slot; 5],
    pub queue: unsafe extern "system" fn(
        *mut c_void,
        *const types::Queue,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
    pub allocator:
        unsafe extern "system" fn(*mut c_void, u32, *const Guid, *mut *mut c_void) -> Hresult,
    pub pipeline: unsafe extern "system" fn(
        *mut c_void,
        *const pipeline::Pipeline,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
    pub compute: Slot,
    pub list: unsafe extern "system" fn(
        *mut c_void,
        u32,
        u32,
        *mut c_void,
        *mut c_void,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
    pub feature: Slot,
    pub heap: unsafe extern "system" fn(
        *mut c_void,
        *const types::Heap,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
    pub increment: unsafe extern "system" fn(*mut c_void, u32) -> u32,
    pub root: unsafe extern "system" fn(
        *mut c_void,
        u32,
        *const c_void,
        usize,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
    pub cbv: Slot,
    pub srv: unsafe extern "system" fn(*mut c_void, *mut c_void, *const types::Shader, types::Cpu),
    pub uav: Slot,
    pub rtv: unsafe extern "system" fn(*mut c_void, *mut c_void, *const types::Target, types::Cpu),
    pub pad1: [Slot; 4],
    pub allocation: unsafe extern "system" fn(
        *mut c_void,
        *mut types::Allocation,
        u32,
        u32,
        *const types::Resource,
    ) -> *mut types::Allocation,
    pub custom: Slot,
    pub resource: unsafe extern "system" fn(
        *mut c_void,
        *const types::Props,
        u32,
        *const types::Resource,
        u32,
        *const types::Clear,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
    pub pad2: [Slot; 8],
    pub fence:
        unsafe extern "system" fn(*mut c_void, u64, u32, *const Guid, *mut *mut c_void) -> Hresult,
    pub removed: unsafe extern "system" fn(*mut c_void) -> Hresult,
    pub footprints: Slot,
    pub query: unsafe extern "system" fn(
        *mut c_void,
        *const types::Query,
        *const Guid,
        *mut *mut c_void,
    ) -> Hresult,
}

#[repr(C)]
pub struct Queue {
    pub base: Unknown,
    pub pad0: [Slot; 7],
    pub execute: unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void),
    pub pad1: [Slot; 3],
    pub signal: unsafe extern "system" fn(*mut c_void, *mut c_void, u64) -> Hresult,
    pub wait: Slot,
    pub frequency: unsafe extern "system" fn(*mut c_void, *mut u64) -> Hresult,
}

#[repr(C)]
pub struct List {
    pub base: Unknown,
    pub pad0: [Slot; 6],
    pub close: unsafe extern "system" fn(*mut c_void) -> Hresult,
    pub reset: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> Hresult,
    pub clearstate: Slot,
    pub draw: unsafe extern "system" fn(*mut c_void, u32, u32, u32, u32),
    pub pad1: [Slot; 2],
    pub copybuffer: unsafe extern "system" fn(*mut c_void, *mut c_void, u64, *mut c_void, u64, u64),
    pub copytexture: unsafe extern "system" fn(
        *mut c_void,
        *const types::Location,
        u32,
        u32,
        u32,
        *const types::Location,
        *const c_void,
    ),
    pub copyresource: unsafe extern "system" fn(*mut c_void, *mut c_void, *mut c_void),
    pub pad2: [Slot; 2],
    pub topology: unsafe extern "system" fn(*mut c_void, u32),
    pub viewports: unsafe extern "system" fn(*mut c_void, u32, *const types::Viewport),
    pub scissors: unsafe extern "system" fn(*mut c_void, u32, *const types::Scissor),
    pub pad3: [Slot; 2],
    pub pipeline: unsafe extern "system" fn(*mut c_void, *mut c_void),
    pub barrier: unsafe extern "system" fn(*mut c_void, u32, *const types::Barrier),
    pub bundle: Slot,
    pub heaps: unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void),
    pub comproot: Slot,
    pub root: unsafe extern "system" fn(*mut c_void, *mut c_void),
    pub comptable: Slot,
    pub table: unsafe extern "system" fn(*mut c_void, u32, types::Gpu),
    pub pad4: [Slot; 3],
    pub constants: unsafe extern "system" fn(*mut c_void, u32, u32, *const c_void, u32),
    pub pad5: [Slot; 3],
    pub srv: unsafe extern "system" fn(*mut c_void, u32, u64),
    pub pad6: [Slot; 5],
    pub targets:
        unsafe extern "system" fn(*mut c_void, u32, *const types::Cpu, i32, *const types::Cpu),
    pub cleardepth: Slot,
    pub clearrtv:
        unsafe extern "system" fn(*mut c_void, types::Cpu, *const f32, u32, *const c_void),
    pub pad7: [Slot; 4],
    pub endquery: unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32),
    pub resolve:
        unsafe extern "system" fn(*mut c_void, *mut c_void, u32, u32, u32, *mut c_void, u64),
}

#[repr(C)]
pub struct Fence {
    pub base: Unknown,
    pub pad0: [Slot; 5],
    pub completed: unsafe extern "system" fn(*mut c_void) -> u64,
    pub event: unsafe extern "system" fn(*mut c_void, u64, Handle) -> Hresult,
}

#[repr(C)]
pub struct Heap {
    pub base: Unknown,
    pub pad0: [Slot; 6],
    pub cpustart: unsafe extern "system" fn(*mut c_void, *mut types::Cpu) -> *mut types::Cpu,
    pub gpustart: unsafe extern "system" fn(*mut c_void, *mut types::Gpu) -> *mut types::Gpu,
}

#[repr(C)]
pub struct Resource {
    pub base: Unknown,
    pub pad0: [Slot; 5],
    pub map: unsafe extern "system" fn(
        *mut c_void,
        u32,
        *const types::Range,
        *mut *mut c_void,
    ) -> Hresult,
    pub unmap: unsafe extern "system" fn(*mut c_void, u32, *const types::Range),
    pub desc: unsafe extern "system" fn(*mut c_void, *mut types::Resource) -> *mut types::Resource,
    pub address: unsafe extern "system" fn(*mut c_void) -> u64,
}
