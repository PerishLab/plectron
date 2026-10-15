use super::*;
use graphics::shared::{Backend, Cpu, Frame, Output, Pass, State as Completion};

impl Window {
    pub fn draw(&mut self, frame: &Frame) -> Result<Completion, Error> {
        self.check()?;
        let result = Output::new(frame.extent(), self.view.scale())
            .and_then(|output| Pass::new(frame, output))
            .and_then(|pass| Cpu.submit(&pass));
        let image = match result {
            Ok(image) => image,
            Err(fault) => return Ok(Completion::Failed(fault)),
        };
        self.install(
            Image {
                extent: crate::shared::Extent {
                    width: image.extent.width,
                    height: image.extent.height,
                },
                pixels: image.pixels,
            },
            Some(frame.clone()),
        )?;
        Ok(Completion::Complete)
    }

    pub fn refresh(&mut self) -> Result<(), Error> {
        if self.closing {
            return Ok(());
        }
        if !self.view.changed() {
            return Ok(());
        }
        let frame = self.frame.clone();
        if let Some(frame) = frame
            && let Completion::Failed(fault) = self.draw(&frame)?
        {
            return Err(super::error(fault));
        }
        Ok(())
    }
}
