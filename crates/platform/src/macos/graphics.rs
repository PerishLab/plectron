use super::{Error, error, window::Window};
use crate::shared::{Image, Receipt, Unit, Visibility};
use graphics::shared::{Compute, Extent, Pass, Scene, Task};
use objc2_quartz_core::CAMetalLayer;

mod accelerator;
mod pixels;
mod recovery;

pub(super) struct Recovery {
    pub unit: Unit,
    pub frame: u64,
    pub image: graphics::shared::Image,
    pub fault: Error,
}

pub enum Graphics {
    Cpu(Compute),
    Pending(shared::Notify),
    Metal(Box<accelerator::Accelerator>),
}

impl Graphics {
    pub fn output(window: &Window, logical: Extent) -> Result<graphics::shared::Output, Error> {
        graphics::shared::Output::new(logical, window.scale()).map_err(error)
    }
    pub fn refresh(
        &mut self,
        unit: Unit,
        window: &Window,
        output: graphics::shared::Output,
    ) -> Result<(), Error> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Ok(()),
            Self::Metal(gpu) => gpu.refresh(unit, window, output),
        }
    }
    pub fn new(name: &str, notify: shared::Notify) -> Result<Self, Error> {
        match name {
            "cpu" => Compute::new(notify).map(Self::Cpu).map_err(error),
            "metal" => Ok(Self::Pending(notify)),
            _ => Err(Error("renderer must be cpu or supported metal".into())),
        }
    }
    pub fn active(&self) -> bool {
        !matches!(self, Self::Cpu(_))
    }
    pub fn prepare(&mut self) -> Result<(), Error> {
        if let Self::Pending(notify) = self {
            *self = Self::Metal(Box::new(accelerator::Accelerator::new(notify.clone())?));
        }
        Ok(())
    }
    pub fn create(
        &mut self,
        unit: Unit,
        window: &Window,
        output: graphics::shared::Output,
    ) -> Result<(), Error> {
        self.prepare()?;
        match self {
            Self::Cpu(_) => Ok(()),
            Self::Pending(_) => unreachable!(),
            Self::Metal(gpu) => gpu.create(unit, window, output),
        }
    }
    pub fn draw(&mut self, unit: Unit, frame: &graphics::shared::Frame) -> Result<(), Error> {
        match self {
            Self::Cpu(_) => Err(Error("CPU scenes use pixel presentation".into())),
            Self::Pending(_) => Err(Error("Metal renderer is not active".into())),
            Self::Metal(gpu) => gpu.draw(unit, frame),
        }
    }
    pub fn image(&mut self, unit: Unit, frame: &Image) -> Result<(), Error> {
        if !self.active() {
            return Ok(());
        }
        let image = graphics::shared::Image {
            extent: Extent {
                width: frame.extent.width,
                height: frame.extent.height,
            },
            pixels: frame.pixels.clone(),
        };
        let scene = Scene::bitmap(&image).map_err(error)?;
        let frame =
            graphics::shared::Frame::new(scene, graphics::shared::Visual::new(image.extent))
                .map_err(error)?;
        match self {
            Self::Cpu(_) => Ok(()),
            Self::Pending(_) => Err(Error("Metal renderer is not active".into())),
            Self::Metal(gpu) => gpu.paint(unit, &frame),
        }
    }
    pub fn close(&mut self, unit: Unit) -> Result<(), Error> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Ok(()),
            Self::Metal(gpu) => gpu.close(unit),
        }
    }
    pub fn poll(&mut self) {
        match self {
            Self::Cpu(_) | Self::Pending(_) => (),
            Self::Metal(gpu) => gpu.poll(),
        }
    }
    pub fn completed(&mut self) -> Vec<Receipt<Error>> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Vec::new(),
            Self::Metal(gpu) => gpu.completed(),
        }
    }
    pub fn recoveries(&mut self) -> Vec<Recovery> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Vec::new(),
            Self::Metal(gpu) => gpu.recoveries(),
        }
    }
    pub fn visible(&mut self) -> Vec<Visibility> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Vec::new(),
            Self::Metal(gpu) => gpu.visible(),
        }
    }
    pub fn compute(&self, pass: Pass) -> Result<Task, Error> {
        match self {
            Self::Cpu(compute) => compute.submit(pass).map_err(error),
            Self::Pending(_) => Err(Error("Metal renderer is not active".into())),
            Self::Metal(gpu) => gpu.compute(pass),
        }
    }
}

fn attach(window: &Window, layer: &CAMetalLayer) -> Result<(), Error> {
    let parent = window.view();
    layer.setFrame(parent.bounds());
    parent.setWantsLayer(true);
    parent
        .layer()
        .ok_or_else(|| Error("window layer unavailable".into()))?
        .addSublayer(layer);
    Ok(())
}
