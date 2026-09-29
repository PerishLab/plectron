use crate::metal::{drawing, output};
use crate::shared::{Backend, Extent, Fault, Image, Pass, State};
use objc2::{
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_metal::*;
use std::{marker::PhantomData, rc::Rc};

pub struct Metal {
    notify: Option<shared::Notify>,
    pub(crate) device: Retained<ProtocolObject<dyn MTLDevice>>,
    pub(crate) queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
    pipeline: drawing::Pipelines,
    store: crate::metal::residency::Store,
    fault: Option<Fault>,
    marker: PhantomData<Rc<()>>,
    setup: Setup,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct Setup {
    pub driver: std::time::Duration,
    pub library: std::time::Duration,
    pub states: std::time::Duration,
}

#[derive(Clone)]
pub struct Work {
    completion: output::Completion,
    pub(crate) texture: Retained<ProtocolObject<dyn MTLTexture>>,
    pub(crate) command: Retained<ProtocolObject<dyn MTLCommandBuffer>>,
    pub(crate) extent: Extent,
    pub(crate) queue: Retained<ProtocolObject<dyn MTLCommandQueue>>,
}

impl Metal {
    pub fn notified(mut self, notify: shared::Notify) -> Self {
        self.notify = Some(notify);
        self
    }
    pub fn new() -> Result<Self, Fault> {
        autoreleasepool(|_| Self::create())
    }

    fn create() -> Result<Self, Fault> {
        let start = std::time::Instant::now();
        let device = MTLCreateSystemDefaultDevice()
            .ok_or_else(|| Fault("Metal device unavailable".into()))?;
        if !device.hasUnifiedMemory() {
            return Err(Fault("this Metal slice requires unified memory".into()));
        }
        let queue = device
            .newCommandQueue()
            .ok_or_else(|| Fault("Metal queue allocation failed".into()))?;
        let driver = start.elapsed();
        let (pipeline, mut setup) = drawing::pipeline(&device)?;
        setup.driver = driver;
        Ok(Self {
            notify: None,
            device,
            queue,
            pipeline,
            store: crate::metal::residency::Store::new(),
            fault: None,
            marker: PhantomData,
            setup,
        })
    }

    pub fn inspect(&self, resource: crate::metal::Resource) -> crate::metal::Residency {
        self.store.report[resource as usize]
    }

    pub fn resources(&self, resource: crate::metal::Resource) -> shared::cache::Snapshot {
        self.store.snapshot(resource)
    }
    pub fn cache(
        &mut self,
        resource: crate::metal::Resource,
        command: shared::cache::Command,
    ) -> Result<(), Fault> {
        self.store.command(resource, command)
    }

    pub fn wait(&mut self, work: &Work, timeout: std::time::Duration) -> Result<(), Fault> {
        if *work.queue != *self.queue {
            return Err(Fault("foreign Metal work".into()));
        }
        work.completion.wait(timeout)?;
        match output::state(&work.command) {
            State::Complete => Ok(()),
            State::Failed(fault) => {
                self.fault = Some(fault.clone());
                Err(fault)
            }
            State::Pending => Err(Fault("Metal signalled incomplete work".into())),
        }
    }
    pub fn batching(&mut self, limit: usize) -> Result<(), Fault> {
        if !(1..=128).contains(&limit) {
            return Err(Fault("path batch limit requires 1 through 128".into()));
        }
        self.store.limit = limit;
        Ok(())
    }
    pub fn batches(&self) -> crate::metal::Batches {
        self.store.batches
    }
    pub fn pool(&mut self) -> &mut crate::metal::pool::Pool {
        &mut self.store.pool
    }
    pub fn name(&self) -> String {
        self.device.name().to_string()
    }
    pub fn setup(&self) -> Setup {
        self.setup
    }

    fn encode(&mut self, frame: &Pass) -> Result<Work, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        self.store.maintain();
        let texture = texture(&self.device, frame.extent())?;
        let command = self
            .queue
            .commandBuffer()
            .ok_or_else(|| Fault("Metal command allocation failed".into()))?;
        let encoded = drawing::encode(
            (&self.device, &self.pipeline),
            frame,
            (&texture, &command),
            &mut self.store,
        );
        self.store.publish();
        if let Err(fault) = encoded {
            self.store.pool.discard();
            return Err(fault);
        }
        let completion = output::Completion::attach(&command, self.notify.clone());
        command.commit();
        self.store.pool.submit(command.clone());
        Ok(Work {
            completion,
            texture,
            command,
            extent: frame.extent(),
            queue: self.queue.clone(),
        })
    }
}

impl Backend for Metal {
    type Work = Work;

    fn submit(&mut self, frame: &Pass) -> Result<Work, Fault> {
        autoreleasepool(|_| self.encode(frame))
    }

    fn poll(&mut self, work: &mut Work) -> Result<State, Fault> {
        let state = autoreleasepool(|_| output::state(&work.command));
        if let State::Failed(fault) = &state {
            self.fault = Some(fault.clone());
            return Err(fault.clone());
        }
        Ok(state)
    }

    fn read(&mut self, work: &Work) -> Result<Image, Fault> {
        if *work.queue != *self.queue {
            return Err(Fault("foreign Metal work".into()));
        }
        autoreleasepool(|_| output::read((&self.device, &self.queue), work))
    }
}

pub(crate) fn texture(
    device: &ProtocolObject<dyn MTLDevice>,
    extent: Extent,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, Fault> {
    extent.area()?;
    if extent.width > 16_384 || extent.height > 16_384 {
        return Err(Fault("texture dimension exceeds profile".into()));
    }
    let descriptor = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2D);
    descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    // SAFETY: Dimensions are positive and bounded by the admitted texture profile.
    unsafe {
        descriptor.setWidth(extent.width);
        descriptor.setHeight(extent.height);
    }
    descriptor.setStorageMode(MTLStorageMode::Private);
    descriptor.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
    device
        .newTextureWithDescriptor(&descriptor)
        .ok_or_else(|| Fault("Metal texture allocation failed".into()))
}

pub(crate) fn upload(
    device: &ProtocolObject<dyn MTLDevice>,
    raster: &crate::shared::Raster,
) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, Fault> {
    let extent = raster.extent();
    if extent.width > 16_384 || extent.height > 16_384 {
        return Err(Fault("image texture dimension exceeds profile".into()));
    }
    let descriptor = MTLTextureDescriptor::new();
    descriptor.setTextureType(MTLTextureType::Type2D);
    descriptor.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
    // SAFETY: Raster dimensions and the corresponding contiguous pixel allocation are validated at construction.
    unsafe {
        descriptor.setWidth(extent.width);
        descriptor.setHeight(extent.height);
    }
    descriptor.setStorageMode(MTLStorageMode::Shared);
    descriptor.setUsage(MTLTextureUsage::ShaderRead);
    let texture = device
        .newTextureWithDescriptor(&descriptor)
        .ok_or_else(|| Fault("image texture allocation failed".into()))?;
    let region = MTLRegion {
        origin: MTLOrigin { x: 0, y: 0, z: 0 },
        size: MTLSize {
            width: extent.width,
            height: extent.height,
            depth: 1,
        },
    };
    let pointer = std::ptr::NonNull::new(raster.pixels().as_ptr().cast_mut().cast())
        .expect("nonempty raster");
    // SAFETY: Metal copies the full live BGRA8 slice with the admitted row stride before this call returns.
    unsafe {
        texture.replaceRegion_mipmapLevel_withBytes_bytesPerRow(
            region,
            0,
            pointer,
            extent.width * 4,
        );
    }
    Ok(texture)
}

impl Work {
    pub fn duration(&self) -> Option<std::time::Duration> {
        if output::state(&self.command) != State::Complete {
            return None;
        }
        let start = self.command.GPUStartTime();
        let end = self.command.GPUEndTime();
        (start > 0.0 && end >= start).then(|| std::time::Duration::from_secs_f64(end - start))
    }
}
