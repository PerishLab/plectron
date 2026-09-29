use crate::metal::{Execution, Work, output, runtime::presentation::Worker};
use crate::shared::{Extent, Fault, Output, Sink, Target};
use objc2::{
    MainThreadMarker,
    rc::{Retained, autoreleasepool},
    runtime::ProtocolObject,
};
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_foundation::NSSize;
use objc2_metal::*;
use objc2_quartz_core::CAMetalLayer;

pub struct Surface {
    notify: Option<shared::Notify>,
    marker: MainThreadMarker,
    layer: Retained<CAMetalLayer>,
    target: Target,
    queue: Option<Retained<ProtocolObject<dyn MTLCommandQueue>>>,
    extent: Extent,
    worker: Option<Worker>,
}

impl Surface {
    pub fn notified(mut self, notify: shared::Notify) -> Self {
        self.notify = Some(notify);
        self
    }
    pub fn new(
        target: Target,
        output: Output,
        attach: impl FnOnce(&CAMetalLayer) -> Result<(), Fault>,
    ) -> Result<Surface, Fault> {
        autoreleasepool(|_| {
            let surface = Self::create(target, output)?;
            attach(&surface.layer)?;
            Ok(surface)
        })
    }

    fn create(target: Target, output: Output) -> Result<Self, Fault> {
        let extent = output.pixels();
        let marker = MainThreadMarker::new()
            .ok_or_else(|| Fault("Metal surface requires main thread".into()))?;
        let layer = CAMetalLayer::new();
        layer.setPixelFormat(MTLPixelFormat::BGRA8Unorm);
        layer.setFramebufferOnly(false);
        layer.setContentsScale(output.scale());
        layer.setDrawableSize(NSSize::new(extent.width as f64, extent.height as f64));
        layer.setAllowsNextDrawableTimeout(true);
        // SAFETY: This is the immutable system sRGB color-space name.
        let space = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))
            .ok_or_else(|| Fault("sRGB color space unavailable".into()))?;
        layer.setColorspace(Some(&space));
        Ok(Surface {
            notify: None,
            marker,
            layer,
            queue: None,
            worker: None,
            target,
            extent,
        })
    }
}

impl Sink<Work> for Surface {
    fn target(&self) -> Target {
        self.target
    }
    fn present(&mut self, work: &Work) -> Result<(), Fault> {
        autoreleasepool(|_| self.draw(work))
    }
    fn completed(&self) -> Option<std::time::Instant> {
        self.worker.as_ref().and_then(Worker::completed)
    }
    fn visibility(&self) -> crate::shared::Visibility {
        self.worker
            .as_ref()
            .map_or(crate::shared::Visibility::Unobserved, Worker::visibility)
    }
    fn cancel(&mut self) {
        self.worker = None;
    }
    fn poll(&mut self) -> Result<crate::shared::State, Fault> {
        self.worker
            .as_mut()
            .ok_or_else(|| Fault("presentation is absent".into()))?
            .poll()
    }
}

impl Sink<Execution> for Surface {
    fn target(&self) -> Target {
        self.target
    }
    fn present(&mut self, work: &Execution) -> Result<(), Fault> {
        autoreleasepool(|_| self.draw(work.artifact()?))
    }
    fn completed(&self) -> Option<std::time::Instant> {
        self.worker.as_ref().and_then(Worker::completed)
    }
    fn visibility(&self) -> crate::shared::Visibility {
        self.worker
            .as_ref()
            .map_or(crate::shared::Visibility::Unobserved, Worker::visibility)
    }
    fn cancel(&mut self) {
        self.worker = None;
    }
    fn poll(&mut self) -> Result<crate::shared::State, Fault> {
        self.worker
            .as_mut()
            .ok_or_else(|| Fault("presentation is absent".into()))?
            .poll()
    }
}

impl Surface {
    fn draw(&mut self, work: &Work) -> Result<(), Fault> {
        if work.extent != self.extent
            || self
                .queue
                .as_ref()
                .is_some_and(|queue| **queue != *work.queue)
        {
            return Err(Fault("foreign or mismatched surface output".into()));
        }
        if output::state(&work.command) != crate::shared::State::Complete {
            return Err(Fault("presentation requires completed rendering".into()));
        }
        if self.worker.is_none() {
            self.layer.setDevice(Some(&work.texture.device()));
            self.queue = Some(work.queue.clone());
            self.worker = Some(Worker::new(self.layer.clone(), self.notify.clone(), true)?);
        }
        self.worker
            .as_mut()
            .expect("configured worker")
            .present(work)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        let _ = self.marker;
        self.worker = None;
        self.layer.removeFromSuperlayer();
    }
}
