use super::{Error, input::Queue};
use crate::shared::{Event, Extent};
use graphics::shared::Output;

pub(super) struct View {
    dpi: u32,
    output: Option<Output>,
    changed: bool,
}

impl View {
    pub fn new(dpi: u32) -> Self {
        Self {
            dpi,
            output: None,
            changed: false,
        }
    }

    pub fn scale(&self) -> f64 {
        f64::from(self.dpi) / 96.0
    }

    pub fn changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn initialize(&mut self, logical: Extent, queue: &Queue) -> Result<(u16, u16), Error> {
        let output = Output::new(
            graphics::shared::Extent {
                width: logical.width,
                height: logical.height,
            },
            self.scale(),
        )
        .map_err(|fault| Error(fault.to_string()))?;
        let pixels = output.pixels();
        queue.density(self.dpi);
        queue.push(Event::Viewport {
            width: logical.width,
            height: logical.height,
            density: (self.dpi, 96),
        })?;
        self.output = Some(output);
        Ok((pixels.width as u16, pixels.height as u16))
    }

    pub fn configure(&mut self, width: u16, height: u16, queue: &Queue) -> Result<(), Error> {
        if width == 0 || height == 0 {
            return Ok(());
        }
        let logical = graphics::shared::Extent {
            width: (u64::from(width) * 96).div_ceil(u64::from(self.dpi)).max(1) as usize,
            height: (u64::from(height) * 96)
                .div_ceil(u64::from(self.dpi))
                .max(1) as usize,
        };
        let output =
            Output::new(logical, self.scale()).map_err(|fault| Error(fault.to_string()))?;
        if self.output.replace(output) != Some(output) {
            self.changed = true;
            queue.push(Event::Viewport {
                width: logical.width,
                height: logical.height,
                density: (self.dpi, 96),
            })?;
        }
        Ok(())
    }
}
