use super::*;
use graphics::shared::{Backend, Cpu, Frame, Output, Pass, State as Completion};

impl Window {
    pub fn draw(&self, frame: &Frame) -> Result<Completion, Error> {
        let _awareness = self.check()?;
        let result = Output::new(frame.extent(), self.state.view.scale())
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
    pub fn refresh(&self) -> Result<(), Error> {
        if self.state.closing.get() {
            return Ok(());
        }
        if !self.state.view.changed() {
            return Ok(());
        }
        let frame = self.state.frame.borrow().clone();
        if let Some(frame) = frame
            && let Completion::Failed(fault) = self.draw(&frame)?
        {
            return Err(super::error(fault));
        }
        Ok(())
    }
}
