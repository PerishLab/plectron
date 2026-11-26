use super::telemetry::Telemetry;
use crate::d3d12::buffer::transition;
use crate::d3d12::device::createfence;
use crate::d3d12::ffi::{Com, Handle, adapter, check, constants, tables, types};
use crate::d3d12::{Work, memory, output};
use crate::shared::{Extent, Fault};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub(super) struct Swapchain {
    chain: Com,
    device: Com,
    queue: Com,
    fence: Com,
    value: u64,
    extent: Extent,
}

// SAFETY: Swapchain is exclusively owned by its single presentation thread; all D3D12/DXGI
// COM calls execute serialized on that thread and the fence is drained before Drop.
unsafe impl Send for Swapchain {}

impl Swapchain {
    pub fn new(handle: Handle, work: &Work, extent: Extent) -> Result<Self, Fault> {
        if handle.is_null() {
            return Err(Fault("DXGI swapchain requires live window handle".into()));
        }
        let factory = adapter::factory()?;
        let desc = types::Chain {
            width: extent.width as u32,
            height: extent.height as u32,
            format: constants::BGRA,
            stereo: 0,
            sample: types::Sample {
                count: 1,
                quality: 0,
            },
            usage: constants::USAGE,
            count: 3,
            scaling: constants::STRETCH,
            effect: constants::FLIPDISCARD,
            alpha: constants::ALPHAIGNORE,
            flags: 0,
        };
        let mut raw = std::ptr::null_mut();
        // SAFETY: factory, queue, and handle are live; desc defines a 3-buffer BGRA8 FLIP_DISCARD swapchain.
        let hr = unsafe {
            let table = factory.vtable::<tables::Factory4>();
            (table.swapchain)(
                factory.raw(),
                work.receipt.queue.raw(),
                handle,
                &desc,
                std::ptr::null(),
                std::ptr::null_mut(),
                &mut raw,
            )
        };
        check(hr, "IDXGIFactory4::CreateSwapChainForHwnd")?;
        // SAFETY: CreateSwapChainForHwnd succeeded and transferred ownership of raw.
        let base = unsafe { Com::own(raw)? };
        // SAFETY: factory and handle are live; NOALTENTER disables automatic fullscreen transitions.
        let _ = unsafe {
            let table = factory.vtable::<tables::Factory4>();
            (table.associate)(factory.raw(), handle, constants::NOALTENTER)
        };
        let chain = base
            .query(&constants::SWAPCHAIN3)
            .ok_or_else(|| Fault("IDXGISwapChain3 unavailable".into()))?;
        let device = work.receipt.signal.device.clone();
        let fence = createfence(&device)?;
        Ok(Self {
            chain,
            device,
            queue: work.receipt.queue.clone(),
            fence,
            value: 0,
            extent,
        })
    }

    pub fn resize(&mut self, extent: Extent) -> Result<(), Fault> {
        self.drain();
        // SAFETY: All backbuffers are released and the GPU fence is drained before ResizeBuffers.
        let hr = unsafe {
            let table = self.chain.vtable::<tables::Chain>();
            (table.resize)(
                self.chain.raw(),
                3,
                extent.width as u32,
                extent.height as u32,
                constants::BGRA,
                0,
            )
        };
        check(hr, "IDXGISwapChain3::ResizeBuffers")?;
        self.extent = extent;
        Ok(())
    }

    pub fn encode(
        &mut self,
        work: &Work,
        stop: &AtomicBool,
    ) -> Result<(Instant, Option<Telemetry>), Fault> {
        if work.receipt.extent != self.extent || work.receipt.queue != self.queue {
            return Err(Fault("swapchain extent or queue changed".into()));
        }
        if stop.load(Ordering::Acquire) {
            return Err(Fault("surface closed".into()));
        }
        // SAFETY: self.chain is a live IDXGISwapChain3 owned by this thread.
        let index = unsafe { (self.chain.vtable::<tables::Chain>().index)(self.chain.raw()) };
        let mut raw = std::ptr::null_mut();
        // SAFETY: index is the active backbuffer index returned by GetCurrentBackBufferIndex.
        let hr = unsafe {
            let table = self.chain.vtable::<tables::Chain>();
            (table.buffer)(self.chain.raw(), index, &constants::RESOURCE, &mut raw)
        };
        check(hr, "IDXGISwapChain3::GetBuffer")?;
        // SAFETY: GetBuffer succeeded and transferred ownership of the backbuffer ID3D12Resource.
        let backbuffer = unsafe { Com::own(raw)? };
        let (pool, list) = memory::Allocator(&self.device).command()?;
        transition(&list, &backbuffer, constants::PRESENT, constants::COPYDEST);
        // SAFETY: work.receipt.texture is in COPY_SOURCE and backbuffer is in COPY_DEST with matching BGRA8 dimensions.
        unsafe {
            let table = list.vtable::<tables::List>();
            (table.copyresource)(list.raw(), backbuffer.raw(), work.receipt.texture.raw());
        }
        transition(&list, &backbuffer, constants::COPYDEST, constants::PRESENT);
        let signal = output::commit(
            (&self.device, &self.queue),
            &list,
            (&self.fence, &mut self.value),
        )?;
        // SAFETY: self.chain is live on this presentation thread; Present(1, 0) queues vsync flip.
        let hr = unsafe { (self.chain.vtable::<tables::Chain>().present)(self.chain.raw(), 1, 0) };
        check(hr, "IDXGISwapChain3::Present")?;
        signal.wait(Duration::from_secs(5))?;
        let _keep = (pool, list, backbuffer);
        let completed = Instant::now();
        Ok((completed, Telemetry::capture(&self.chain, completed)))
    }

    pub fn drain(&mut self) {
        if self.value == 0 {
            return;
        }
        let signal = output::Signal {
            device: self.device.clone(),
            fence: self.fence.clone(),
            value: self.value,
        };
        let _ = signal.wait(Duration::from_secs(5));
    }
}
