use super::Error;
use crate::shared::{Event, Extent};
use graphics::shared::Output;
use std::collections::VecDeque;

pub(super) struct View {
    dpi: u32,
    output: Option<Output>,
    changed: bool,
    events: VecDeque<Event>,
}

impl View {
    pub fn new(dpi: u32) -> Self {
        Self {
            dpi,
            output: None,
            changed: false,
            events: VecDeque::new(),
        }
    }

    pub fn scale(&self) -> f64 {
        f64::from(self.dpi) / 96.0
    }

    pub fn changed(&mut self) -> bool {
        std::mem::take(&mut self.changed)
    }

    pub fn initialize(&mut self, logical: Extent) -> Result<(u16, u16), Error> {
        let output = Output::new(
            graphics::shared::Extent {
                width: logical.width,
                height: logical.height,
            },
            self.scale(),
        )
        .map_err(|fault| Error(fault.to_string()))?;
        let pixels = output.pixels();
        self.push(Event::Viewport {
            width: logical.width,
            height: logical.height,
            density: (self.dpi, 96),
        })?;
        self.output = Some(output);
        Ok((pixels.width as u16, pixels.height as u16))
    }

    pub fn configure(&mut self, width: u16, height: u16) -> Result<(), Error> {
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
        if self.output != Some(output) {
            self.output = Some(output);
            self.changed = true;
        }
        Ok(())
    }

    fn push(&mut self, event: Event) -> Result<(), Error> {
        if matches!(&event, Event::Viewport { .. })
            && matches!(self.events.back(), Some(Event::Viewport { .. }))
        {
            self.events.pop_back();
        }
        if self.events.len() == 512 {
            return Err(Error("native input queue exhausted".into()));
        }
        self.events.push_back(event);
        Ok(())
    }

    pub fn drain(&mut self) -> Vec<Event> {
        let count = self.events.len().min(16);
        self.events.drain(..count).collect()
    }

    pub fn pending(&self) -> bool {
        !self.events.is_empty()
    }
}
