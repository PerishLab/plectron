use crate::d3d12::buffer::Arena;
use crate::d3d12::ffi::{Com, check, constants, tables, types};
use crate::shared::Fault;

struct Descriptor {
    heap: Com,
    cpu: usize,
    gpu: u64,
    step: usize,
}

pub(crate) struct Frame {
    pub list: Com,
    pub arena: Arena,
    pub rtvheap: Com,
    pub srvheap: Com,
    rtvbase: usize,
    srvcpu: usize,
    srvgpu: u64,
    rtvstep: usize,
    srvstep: usize,
    rtvused: usize,
    srvused: usize,
    rtvlimit: usize,
    srvlimit: usize,
    pub retained: Vec<Com>,
}

impl Frame {
    pub fn new(device: &Com, list: Com) -> Result<Self, Fault> {
        let rtv = heap(device, constants::RTVHEAP, 256, 0)?;
        let srv = heap(device, constants::SRVHEAP, 4096, constants::VISIBLE)?;
        Ok(Self {
            list,
            arena: Arena::new(device.clone()),
            rtvheap: rtv.heap,
            srvheap: srv.heap,
            rtvbase: rtv.cpu,
            srvcpu: srv.cpu,
            srvgpu: srv.gpu,
            rtvstep: rtv.step,
            srvstep: srv.step,
            rtvused: 0,
            srvused: 0,
            rtvlimit: 256,
            srvlimit: 4096,
            retained: Vec::new(),
        })
    }

    pub fn rtv(&mut self, device: &Com, resource: &Com) -> Result<types::Cpu, Fault> {
        if self.rtvused >= self.rtvlimit {
            return Err(Fault(
                "D3D12 render target descriptor budget exhausted".into(),
            ));
        }
        let handle = types::Cpu {
            ptr: self.rtvbase + self.rtvused * self.rtvstep,
        };
        self.rtvused += 1;
        let desc = types::Target {
            format: constants::BGRA,
            dimension: constants::VIEWTEXTURE,
            mip: 0,
            plane: 0,
            pad: 0,
        };
        // SAFETY: device and resource are live; handle points within our owned RTV descriptor heap.
        unsafe {
            let table = device.vtable::<tables::Device>();
            (table.rtv)(device.raw(), resource.raw(), &desc, handle);
        }
        Ok(handle)
    }

    pub fn srv(&mut self, device: &Com, resource: &Com) -> Result<types::Gpu, Fault> {
        if self.srvused >= self.srvlimit {
            return Err(Fault(
                "D3D12 shader resource descriptor budget exhausted".into(),
            ));
        }
        let cpu = types::Cpu {
            ptr: self.srvcpu + self.srvused * self.srvstep,
        };
        let gpu = types::Gpu {
            ptr: self.srvgpu + (self.srvused * self.srvstep) as u64,
        };
        self.srvused += 1;
        let desc = types::Shader {
            format: constants::BGRA,
            dimension: constants::VIEWTEXTURE,
            mapping: constants::MAPPING,
            data: types::View {
                texture: types::Texture {
                    detailed: 0,
                    levels: 1,
                    plane: 0,
                    clamp: 0.0,
                },
            },
        };
        // SAFETY: device and resource are live; cpu points inside our owned shader-visible SRV heap.
        unsafe {
            let table = device.vtable::<tables::Device>();
            (table.srv)(device.raw(), resource.raw(), &desc, cpu);
        }
        Ok(gpu)
    }
}

fn heap(device: &Com, kind: u32, count: u32, flags: u32) -> Result<Descriptor, Fault> {
    let desc = types::Heap {
        kind,
        count,
        flags,
        mask: 0,
    };
    let mut raw = std::ptr::null_mut();
    // SAFETY: device is live and desc defines an admitted descriptor heap.
    let hr = unsafe {
        let table = device.vtable::<tables::Device>();
        (table.heap)(device.raw(), &desc, &constants::HEAP, &mut raw)
    };
    check(hr, "ID3D12Device::CreateDescriptorHeap")?;
    // SAFETY: CreateDescriptorHeap succeeded and returned an owned ID3D12DescriptorHeap.
    let heap = unsafe { Com::own(raw)? };
    let mut cpu = types::Cpu::default();
    let mut gpu = types::Gpu::default();
    // SAFETY: On _WIN32, GetCPU/GPUDescriptorHandleForHeapStart write to hidden out pointers.
    let step = unsafe {
        let htable = heap.vtable::<tables::Heap>();
        (htable.cpustart)(heap.raw(), &mut cpu);
        if flags != 0 {
            (htable.gpustart)(heap.raw(), &mut gpu);
        }
        let dtable = device.vtable::<tables::Device>();
        (dtable.increment)(device.raw(), kind) as usize
    };
    Ok(Descriptor {
        heap,
        cpu: cpu.ptr,
        gpu: gpu.ptr,
        step,
    })
}
