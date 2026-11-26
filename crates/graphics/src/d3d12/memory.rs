use crate::d3d12::buffer::{Arena, transition};
use crate::d3d12::ffi::{Com, check, constants, tables, types};
use crate::shared::{Extent, Fault, Raster};

#[derive(Clone)]
pub(crate) struct Texture {
    pub resource: Com,
    pub device: Com,
    pub extent: Extent,
    pub allocated: usize,
}

pub(crate) struct Allocator<'a>(pub &'a Com);

impl Allocator<'_> {
    pub fn allocation(&self, desc: &types::Resource) -> usize {
        let mut info = types::Allocation::default();
        // SAFETY: On _WIN32, GetResourceAllocationInfo writes into the hidden out pointer info.
        unsafe {
            let table = self.0.vtable::<tables::Device>();
            (table.allocation)(self.0.raw(), &mut info, 0, 1, desc);
        }
        info.bytes as usize
    }

    pub fn bufferinfo(&self, bytes: usize, mode: (u32, u32)) -> Result<(Com, usize), Fault> {
        let desc = types::Resource {
            dimension: constants::BUFFER,
            alignment: 0,
            width: bytes.max(4) as u64,
            height: 1,
            depth: 1,
            mips: 1,
            format: 0,
            sample: types::Sample {
                count: 1,
                quality: 0,
            },
            layout: constants::ROWMAJOR,
            flags: 0,
        };
        let props = types::Props {
            kind: mode.0,
            ..types::Props::default()
        };
        let mut out = std::ptr::null_mut();
        // SAFETY: device is live; props and desc describe an admitted committed buffer resource.
        let hr = unsafe {
            let table = self.0.vtable::<tables::Device>();
            (table.resource)(
                self.0.raw(),
                &props,
                0,
                &desc,
                mode.1,
                std::ptr::null(),
                &constants::RESOURCE,
                &mut out,
            )
        };
        check(hr, "ID3D12Device::CreateCommittedResource")?;
        // SAFETY: CreateCommittedResource succeeded and wrote an owned ID3D12Resource into out.
        let resource = unsafe { Com::own(out)? };
        Ok((resource, self.allocation(&desc)))
    }

    pub fn buffer(&self, bytes: usize, mode: (u32, u32)) -> Result<Com, Fault> {
        self.bufferinfo(bytes, mode).map(|(resource, _)| resource)
    }

    pub fn texture(&self, extent: Extent) -> Result<Texture, Fault> {
        extent.area()?;
        if extent.width > 16_384 || extent.height > 16_384 {
            return Err(Fault("texture dimension exceeds profile".into()));
        }
        let desc = types::Resource {
            dimension: constants::TEXTURE,
            alignment: 0,
            width: extent.width as u64,
            height: extent.height as u32,
            depth: 1,
            mips: 1,
            format: constants::BGRA,
            sample: types::Sample {
                count: 1,
                quality: 0,
            },
            layout: 0,
            flags: constants::ALLOWTARGET,
        };
        let props = types::Props {
            kind: constants::DEFAULT,
            ..types::Props::default()
        };
        let mut out = std::ptr::null_mut();
        // SAFETY: Dimensions are validated; props and desc define a BGRA8 DEFAULT render target.
        let hr = unsafe {
            let table = self.0.vtable::<tables::Device>();
            (table.resource)(
                self.0.raw(),
                &props,
                0,
                &desc,
                constants::TARGET,
                std::ptr::null(),
                &constants::RESOURCE,
                &mut out,
            )
        };
        check(hr, "ID3D12Device::CreateCommittedResource")?;
        // SAFETY: CreateCommittedResource succeeded and transferred ownership of out.
        let resource = unsafe { Com::own(out)? };
        Ok(Texture {
            resource,
            device: self.0.clone(),
            extent,
            allocated: self.allocation(&desc),
        })
    }

    pub fn command(&self) -> Result<(Com, Com), Fault> {
        let mut alloc = std::ptr::null_mut();
        // SAFETY: device is a live ID3D12Device creating a direct command allocator.
        let hr = unsafe {
            let table = self.0.vtable::<tables::Device>();
            (table.allocator)(self.0.raw(), 0, &constants::ALLOCATOR, &mut alloc)
        };
        check(hr, "ID3D12Device::CreateCommandAllocator")?;
        // SAFETY: CreateCommandAllocator succeeded and transferred ownership.
        let allocator = unsafe { Com::own(alloc)? };
        let mut raw = std::ptr::null_mut();
        // SAFETY: device and allocator are live; null PSO leaves initial state unset.
        let hr = unsafe {
            let table = self.0.vtable::<tables::Device>();
            (table.list)(
                self.0.raw(),
                0,
                0,
                allocator.raw(),
                std::ptr::null_mut(),
                &constants::LIST,
                &mut raw,
            )
        };
        check(hr, "ID3D12Device::CreateCommandList")?;
        // SAFETY: CreateCommandList succeeded and transferred ownership.
        let list = unsafe { Com::own(raw)? };
        Ok((allocator, list))
    }

    pub fn upload(&self, list: &Com, arena: &mut Arena, raster: &Raster) -> Result<Texture, Fault> {
        let extent = raster.extent();
        if extent.width > 16_384 || extent.height > 16_384 {
            return Err(Fault("image texture dimension exceeds profile".into()));
        }
        let desc = types::Resource {
            dimension: constants::TEXTURE,
            alignment: 0,
            width: extent.width as u64,
            height: extent.height as u32,
            depth: 1,
            mips: 1,
            format: constants::BGRA,
            sample: types::Sample {
                count: 1,
                quality: 0,
            },
            layout: 0,
            flags: 0,
        };
        let props = types::Props {
            kind: constants::DEFAULT,
            ..types::Props::default()
        };
        let mut out = std::ptr::null_mut();
        // SAFETY: Raster extent is validated; resource starts in COPY_DEST for staging upload.
        let hr = unsafe {
            let table = self.0.vtable::<tables::Device>();
            (table.resource)(
                self.0.raw(),
                &props,
                0,
                &desc,
                constants::COPYDEST,
                std::ptr::null(),
                &constants::RESOURCE,
                &mut out,
            )
        };
        check(hr, "ID3D12Device::CreateCommittedResource")?;
        // SAFETY: CreateCommittedResource succeeded and returned an owned ID3D12Resource.
        let resource = unsafe { Com::own(out)? };
        let stride = (extent.width * 4).div_ceil(256) * 256;
        let mut staging = vec![0u8; stride * extent.height];
        let pixels = raster.pixels();
        for y in 0..extent.height {
            let row = &pixels[y * extent.width..(y + 1) * extent.width];
            let dest = &mut staging[y * stride..y * stride + extent.width * 4];
            for (slot, pixel) in dest.chunks_exact_mut(4).zip(row) {
                slot.copy_from_slice(&pixel.to_le_bytes());
            }
        }
        let (buffer, offset, _) = arena.write(&staging, 512)?;
        let destloc = types::Location {
            resource: resource.raw(),
            kind: 0,
            data: types::Copy { index: 0 },
        };
        let srcloc = types::Location {
            resource: buffer.raw(),
            kind: 1,
            data: types::Copy {
                placed: types::Placed {
                    offset,
                    footprint: types::Footprint {
                        format: constants::BGRA,
                        width: extent.width as u32,
                        height: extent.height as u32,
                        depth: 1,
                        pitch: stride as u32,
                    },
                },
            },
        };
        // SAFETY: buffer is in GENERIC_READ at 512-byte-aligned offset; resource is in COPY_DEST.
        unsafe {
            let table = list.vtable::<tables::List>();
            (table.copytexture)(list.raw(), &destloc, 0, 0, 0, &srcloc, std::ptr::null());
        }
        transition(list, &resource, constants::COPYDEST, constants::PIXEL);
        Ok(Texture {
            resource,
            device: self.0.clone(),
            extent,
            allocated: self.allocation(&desc),
        })
    }
}
