use super::{Error, error, window::Window};
use crate::shared::{Image, Receipt, Unit, Visibility};
use graphics::shared::{Compute, Extent, Frame, Output, Scene, Visual};

mod accelerator;
mod recovery;
#[cfg(test)]
mod tests;

pub(super) struct Recovery {
    pub unit: Unit,
    pub frame: u64,
    pub image: graphics::shared::Image,
    pub fault: Error,
}

pub enum Graphics {
    Cpu(Compute),
    Pending(shared::Notify),
    D3d12(Box<accelerator::Accelerator>),
}

impl Graphics {
    pub fn output(window: &Window, logical: Extent) -> Result<Output, Error> {
        Output::new(logical, window.scale()).map_err(error)
    }

    pub fn new(name: &str, notify: shared::Notify) -> Result<Self, Error> {
        match name {
            "cpu" => Compute::new(notify).map(Self::Cpu).map_err(error),
            "d3d12" => Ok(Self::Pending(notify)),
            _ => Err(Error("renderer must be cpu or supported d3d12".into())),
        }
    }

    pub fn active(&self) -> bool {
        !matches!(self, Self::Cpu(_))
    }

    pub fn prepare(&mut self) -> Result<(), Error> {
        if let Self::Pending(notify) = self {
            *self = Self::D3d12(Box::new(accelerator::Accelerator::new(notify.clone())?));
        }
        Ok(())
    }

    pub fn create(&mut self, unit: Unit, window: &Window, output: Output) -> Result<(), Error> {
        self.prepare()?;
        match self {
            Self::Cpu(_) => Ok(()),
            Self::Pending(_) => unreachable!(),
            Self::D3d12(gpu) => gpu.create(unit, window, output),
        }
    }

    pub fn refresh(&mut self, unit: Unit, window: &Window) -> Result<(), Error> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Ok(()),
            Self::D3d12(gpu) => gpu.refresh(unit, window),
        }
    }

    pub fn draw(&mut self, unit: Unit, window: &Window, frame: &Frame) -> Result<(), Error> {
        match self {
            Self::Cpu(_) => Err(Error("CPU scenes use pixel presentation".into())),
            Self::Pending(_) => Err(Error("D3D12 renderer is not active".into())),
            Self::D3d12(gpu) => gpu.draw(unit, window, frame),
        }
    }

    pub fn image(&mut self, unit: Unit, window: &Window, frame: &Image) -> Result<(), Error> {
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
        let frame = Frame::new(scene, Visual::new(image.extent)).map_err(error)?;
        match self {
            Self::Cpu(_) => Ok(()),
            Self::Pending(_) => Err(Error("D3D12 renderer is not active".into())),
            Self::D3d12(gpu) => gpu.paint(unit, window, &frame),
        }
    }

    pub fn close(&mut self, unit: Unit) -> Result<(), Error> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Ok(()),
            Self::D3d12(gpu) => gpu.close(unit),
        }
    }

    pub fn poll(&mut self) {
        if let Self::D3d12(gpu) = self {
            gpu.poll();
        }
    }

    pub fn completed(&mut self) -> Vec<Receipt<Error>> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Vec::new(),
            Self::D3d12(gpu) => gpu.completed(),
        }
    }

    pub fn recoveries(&mut self) -> Vec<Recovery> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Vec::new(),
            Self::D3d12(gpu) => gpu.recoveries(),
        }
    }

    pub fn visible(&mut self) -> Vec<Visibility> {
        match self {
            Self::Cpu(_) | Self::Pending(_) => Vec::new(),
            Self::D3d12(gpu) => gpu.visible(),
        }
    }

    pub fn awaiting(&self) -> bool {
        match self {
            Self::Cpu(_) | Self::Pending(_) => false,
            Self::D3d12(gpu) => gpu.awaiting(),
        }
    }

    pub fn compute(&self) -> Result<&Compute, Error> {
        match self {
            Self::Cpu(compute) => Ok(compute),
            Self::Pending(_) => Err(Error("D3D12 renderer is not active".into())),
            Self::D3d12(gpu) => Ok(gpu.compute()),
        }
    }

    #[cfg(test)]
    pub(super) fn inject(&mut self, unit: Unit, frame: Frame, fault: Error) {
        if let Self::D3d12(gpu) = self {
            gpu.inject(unit, frame, fault);
        }
    }
}
