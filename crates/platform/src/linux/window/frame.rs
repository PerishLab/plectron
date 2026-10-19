use super::*;
use crate::shared::{Receipt, Unit};
use graphics::shared::{Compute, Fault, Frame, Output, State as Completion, Task};
use std::time::Instant;

struct Job {
    frame: u64,
    output: Output,
    task: Task,
}

pub(super) struct Done {
    pub frame: u64,
    pub produced: Option<Instant>,
    pub result: Result<(), Error>,
}

#[derive(Default)]
pub(super) struct Raster {
    frame: Option<Frame>,
    target: Option<Output>,
    output: Option<Output>,
    job: Option<Job>,
    owed: Option<u64>,
    installed: Option<u64>,
    done: Vec<Done>,
    fault: Option<Error>,
}

impl Raster {
    pub fn draw(&mut self, frame: &Frame, output: Output, compute: &Compute) -> Completion {
        if self.owed != Some(frame.id()) {
            self.retire();
        }
        match self.submit(frame, output, compute) {
            Ok(()) => {
                self.frame = Some(frame.clone());
                self.target = Some(output);
                self.owed = Some(frame.id());
                Completion::Pending
            }
            Err(fault) => Completion::Failed(fault),
        }
    }

    fn submit(&mut self, frame: &Frame, output: Output, compute: &Compute) -> Result<(), Fault> {
        self.job = None;
        let task = frame
            .prepare(output)
            .and_then(|pass| compute.submit(pass))?;
        self.job = Some(Job {
            frame: frame.id(),
            output,
            task,
        });
        Ok(())
    }

    pub fn refresh(&mut self, scale: f64, compute: &Compute) -> Result<bool, Error> {
        let Some(frame) = self.frame.clone() else {
            return Ok(false);
        };
        let output = Output::new(frame.extent(), scale).map_err(error)?;
        self.target = Some(output);
        if self.job.is_some() {
            return Ok(false);
        }
        if self.output == Some(output) {
            return Ok(true);
        }
        self.submit(&frame, output, compute).map_err(error)?;
        Ok(false)
    }

    pub fn poll(&mut self, compute: &Compute) -> Option<graphics::shared::Image> {
        let mut job = self.job.take()?;
        let image = match job.task.take() {
            Ok(Some(image)) => image,
            Ok(None) => {
                self.job = Some(job);
                return None;
            }
            Err(fault) => {
                self.fail(job.frame, error(fault));
                return None;
            }
        };
        if self.target != Some(job.output) {
            let (Some(frame), Some(output)) = (self.frame.clone(), self.target) else {
                return None;
            };
            if let Err(fault) = self.submit(&frame, output, compute) {
                self.fail(job.frame, error(fault));
            }
            return None;
        }
        self.output = Some(job.output);
        self.installed = Some(job.frame);
        Some(image)
    }

    fn fail(&mut self, frame: u64, fault: Error) {
        if self.owed == Some(frame) {
            self.owed = None;
            self.done.push(Done {
                frame,
                produced: None,
                result: Err(fault),
            });
        } else {
            self.fault = Some(fault);
        }
    }

    pub fn present(&mut self, result: Result<(), Error>) -> Result<(), Error> {
        let Some(frame) = self.installed.take() else {
            return Ok(());
        };
        if self.owed != Some(frame) {
            return result;
        }
        self.owed = None;
        self.done.push(Done {
            frame,
            produced: result.is_ok().then(Instant::now),
            result,
        });
        Ok(())
    }

    fn retire(&mut self) {
        if let Some(frame) = self.owed.take() {
            self.done.push(Done {
                frame,
                produced: None,
                result: Err(Error("frame superseded before production".into())),
            });
        }
    }

    pub fn clear(&mut self) {
        self.retire();
        self.frame = None;
        self.target = None;
        self.output = None;
        self.job = None;
        self.installed = None;
    }

    #[cfg(test)]
    pub fn pending(&self) -> bool {
        self.job.is_some()
    }

    #[cfg(test)]
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }
}

impl Window {
    pub fn draw(&mut self, frame: &Frame, compute: &Compute) -> Result<Completion, Error> {
        self.check()?;
        Ok(match Output::new(frame.extent(), self.view.scale()) {
            Ok(output) => self.raster.draw(frame, output, compute),
            Err(fault) => Completion::Failed(fault),
        })
    }

    pub fn refresh(&mut self, compute: &Compute) -> Result<(), Error> {
        if let Some(fault) = self.raster.fault.take() {
            return Err(fault);
        }
        if self.closing {
            return Ok(());
        }
        if !self.view.changed() {
            return Ok(());
        }
        if self.raster.refresh(self.view.scale(), compute)? {
            let result = self.present.blit(self.image.as_ref());
            self.settle(result)?;
        }
        Ok(())
    }

    pub fn completed(&mut self, unit: Unit, compute: &Compute) -> Vec<Receipt<Error>> {
        if let Some(image) = self.raster.poll(compute) {
            let result = self.install(Image {
                extent: crate::shared::Extent {
                    width: image.extent.width,
                    height: image.extent.height,
                },
                pixels: image.pixels,
            });
            let result = match result {
                Ok(()) if self.destroyed => Err(Error(
                    "native window was destroyed before presentation".into(),
                )),
                result => result,
            };
            if let Err(fault) = self.raster.present(result)
                && !self.destroyed
            {
                self.raster.fault = Some(fault);
            }
        }
        std::mem::take(&mut self.raster.done)
            .into_iter()
            .map(|done| Receipt {
                unit,
                frame: done.frame,
                produced: done.produced,
                visible: None,
                drops: 0,
                result: done.result,
            })
            .collect()
    }
}

#[path = "../../../tests/linux/frame.rs"]
mod checks;
