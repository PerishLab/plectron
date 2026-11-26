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

    pub fn present(&mut self, result: Result<(), Error>) -> Result<Option<(u64, Instant)>, Error> {
        let Some(frame) = self.installed.take() else {
            return Ok(None);
        };
        if self.owed != Some(frame) {
            return result.map(|()| None);
        }
        self.owed = None;
        let produced = result.is_ok().then(Instant::now);
        self.done.push(Done {
            frame,
            produced,
            result,
        });
        Ok(produced.map(|at| (frame, at)))
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
    pub fn frame(&self) -> Option<&Frame> {
        self.frame.as_ref()
    }
}

impl Window {
    pub fn draw(&self, frame: &Frame, compute: &Compute) -> Result<Completion, Error> {
        let _awareness = self.check()?;
        Ok(match Output::new(frame.extent(), self.state.view.scale()) {
            Ok(output) => self.state.raster.borrow_mut().draw(frame, output, compute),
            Err(fault) => Completion::Failed(fault),
        })
    }

    pub fn refresh(&self, compute: &Compute) -> Result<(), Error> {
        if let Some(fault) = self.state.raster.borrow_mut().fault.take() {
            return Err(fault);
        }
        if self.state.closing.get() {
            return Ok(());
        }
        if !self.state.view.changed() {
            return Ok(());
        }
        let same = self
            .state
            .raster
            .borrow_mut()
            .refresh(self.state.view.scale(), compute)?;
        if same {
            self.repaint()?;
        }
        Ok(())
    }

    pub fn completed(
        &self,
        unit: Unit,
        compute: &Compute,
        timing: &super::super::timing::Timing,
    ) -> Vec<Receipt<Error>> {
        let ready = self.state.raster.borrow_mut().poll(compute);
        if let Some(image) = ready {
            self.finish(image, timing);
        }
        let done = std::mem::take(&mut self.state.raster.borrow_mut().done);
        done.into_iter()
            .map(|item| Receipt {
                unit,
                frame: item.frame,
                produced: item.produced,
                visible: None,
                drops: 0,
                result: item.result,
            })
            .collect()
    }

    fn finish(&self, image: graphics::shared::Image, timing: &super::super::timing::Timing) {
        let result = self.install(
            Image {
                extent: crate::shared::Extent {
                    width: image.extent.width,
                    height: image.extent.height,
                },
                pixels: image.pixels,
            },
            true,
        );
        let settled = self.state.raster.borrow_mut().present(result);
        match settled {
            Ok(Some((frame, produced))) if self.state.shown() => {
                if let Some(current) = timing.current() {
                    self.state
                        .sightings
                        .borrow_mut()
                        .watch(frame, current.refresh, produced);
                }
            }
            Ok(_) => {}
            Err(fault) if !self.state.destroyed.get() => {
                self.state.raster.borrow_mut().fault = Some(fault);
            }
            Err(_) => {}
        }
    }
}
