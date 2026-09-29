use super::super::{Canvas, Error, error};
use super::Graphics;
use crate::shared::{Extent, Image, Unit};
use graphics::shared::{Frame, Output, Scene, Visual};

impl Canvas {
    pub(in crate::macos) fn refresh(
        &mut self,
        unit: Unit,
        graphics: &mut Graphics,
    ) -> Result<(), Error> {
        let output = Graphics::output(&self.window, self.output.logical())?;
        if output == self.output {
            return Ok(());
        }
        if self.accelerated {
            graphics.refresh(unit, &self.window, output)?;
        }
        self.resize(output, self.accelerated, graphics)
    }
    pub(in crate::macos) fn draw(
        &mut self,
        frame: &Frame,
        graphics: &Graphics,
    ) -> Result<(), Error> {
        let started = std::time::Instant::now();
        let pass = frame.prepare(self.output).map_err(error)?;
        let prepared = started.elapsed();
        if super::super::ime::trace() {
            eprintln!(
                "[render] frame={} prepare_ms={:.2} queued=1",
                frame.id(),
                prepared.as_secs_f64() * 1000.0
            );
        }
        self.task = Some((
            frame.id(),
            std::time::Instant::now(),
            graphics.compute(pass)?,
        ));
        Ok(())
    }
    pub(in crate::macos) fn raster(&mut self, image: graphics::shared::Image) -> Result<(), Error> {
        if image.extent != self.output.pixels() {
            return Err(Error("CPU raster does not match the window output".into()));
        }
        self.frame = Image {
            extent: Extent {
                width: image.extent.width,
                height: image.extent.height,
            },
            pixels: image.pixels,
        };
        self.dirty = true;
        Ok(())
    }
    pub(in crate::macos) fn image(
        &mut self,
        image: Image,
        graphics: &Graphics,
    ) -> Result<(), Error> {
        let image = graphics::shared::Image {
            extent: graphics::shared::Extent {
                width: image.extent.width,
                height: image.extent.height,
            },
            pixels: image.pixels,
        };
        let scene = Scene::bitmap(&image).map_err(error)?;
        let frame = Frame::new(scene, Visual::new(image.extent)).map_err(error)?;
        self.draw(&frame, graphics)?;
        self.scene = Some(frame);
        Ok(())
    }
    pub(in crate::macos) fn resize(
        &mut self,
        output: Output,
        accelerated: bool,
        graphics: &Graphics,
    ) -> Result<(), Error> {
        self.output = output;
        if !accelerated && let Some(frame) = self.scene.clone() {
            self.draw(&frame, graphics)?;
        }
        Ok(())
    }
    pub(in crate::macos) fn poll(&mut self, accelerated: bool) -> Result<(), Error> {
        if accelerated {
            return Ok(());
        }
        if let Some((frame, started, mut task)) = self.task.take() {
            let image = match task.take() {
                Ok(Some(image)) => image,
                Ok(None) => {
                    self.task = Some((frame, started, task));
                    return Ok(());
                }
                Err(fault) => {
                    self.pending = Some(frame);
                    return Err(error(fault));
                }
            };
            if super::super::ime::trace() {
                eprintln!(
                    "[render] frame={} compute_ms={:.2}",
                    frame,
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
            self.pending = Some(frame);
            self.raster(image)?;
        }
        if self.dirty {
            let extent = self.frame.extent;
            let started = std::time::Instant::now();
            self.window
                .present(&self.frame.pixels, extent.width, extent.height)?;
            let elapsed = started.elapsed();
            if super::super::ime::trace() {
                eprintln!(
                    "[window] copy_ms={:.2} bytes={}",
                    elapsed.as_secs_f64() * 1000.0,
                    self.frame.pixels.len() * std::mem::size_of::<u32>()
                );
            }
            self.dirty = false;
        }
        Ok(())
    }
}
