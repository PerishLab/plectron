use super::super::{Error, error, window::Window};
use super::Recovery;
use crate::shared::{Receipt, Unit, Visibility};
use graphics::metal::{Executor, Surface};
use graphics::shared::{Compute, Driver, Frame, Output, Pass, Target, Task};
use std::collections::{BTreeMap, BTreeSet};

pub struct Accelerator {
    targets: BTreeMap<Unit, Destination>,
    driver: Driver<Executor, Surface>,
    receipts: Vec<Receipt<Error>>,
    visibility: Vec<Visibility>,
    recoveries: Vec<Recovery>,
    fallbacks: Vec<Fallback>,
    compute: Compute,
    tracked: BTreeMap<Unit, u64>,
    awaiting: BTreeSet<(Unit, u64)>,
    notify: shared::Notify,
}

struct Destination {
    target: Target,
    output: Output,
    frame: Option<Frame>,
}

struct Fallback {
    unit: Unit,
    frame: u64,
    fault: Error,
    task: Task,
}
impl Accelerator {
    pub fn new(notify: shared::Notify) -> Result<Self, Error> {
        let metal = Executor::new(notify.clone()).map_err(error)?;
        let compute = Compute::new(notify.clone()).map_err(error)?;
        Ok(Self {
            targets: BTreeMap::new(),
            driver: Driver::new(metal),
            receipts: Vec::new(),
            visibility: Vec::new(),
            recoveries: Vec::new(),
            fallbacks: Vec::new(),
            compute,
            tracked: BTreeMap::new(),
            awaiting: BTreeSet::new(),
            notify,
        })
    }
    pub fn create(&mut self, unit: Unit, window: &Window, output: Output) -> Result<(), Error> {
        let target = self.bind(window, output)?;
        self.targets.insert(
            unit,
            Destination {
                target,
                output,
                frame: None,
            },
        );
        Ok(())
    }
    fn bind(&mut self, window: &Window, output: Output) -> Result<Target, Error> {
        let target = self.driver.output(output).map_err(error)?;
        let result = Surface::new(target, output, |layer| {
            super::attach(window, layer).map_err(|failure| graphics::shared::Fault(failure.0))
        })
        .and_then(|surface| self.driver.attach(surface.notified(self.notify.clone())))
        .map_err(error);
        if let Err(fault) = result {
            self.driver.close(target).map_err(error)?;
            return Err(error(fault));
        }
        Ok(target)
    }
    pub fn refresh(&mut self, unit: Unit, window: &Window, output: Output) -> Result<(), Error> {
        let current = self
            .targets
            .get(&unit)
            .ok_or_else(|| Error("Metal surface absent".into()))?;
        if current.output == output {
            return Ok(());
        }
        let frame = current.frame.clone();
        self.driver.close(current.target).map_err(error)?;
        let target = self.bind(window, output)?;
        self.targets.insert(
            unit,
            Destination {
                target,
                output,
                frame: None,
            },
        );
        if let Some(frame) = frame {
            self.paint(unit, &frame)?;
        }
        Ok(())
    }
    pub fn draw(&mut self, unit: Unit, frame: &graphics::shared::Frame) -> Result<(), Error> {
        self.paint(unit, frame)?;
        self.tracked.insert(unit, frame.id());
        Ok(())
    }
    pub fn compute(&self, pass: Pass) -> Result<Task, Error> {
        self.compute.submit(pass).map_err(error)
    }
    pub fn paint(&mut self, unit: Unit, frame: &graphics::shared::Frame) -> Result<(), Error> {
        let target = self
            .targets
            .get_mut(&unit)
            .ok_or_else(|| Error("Metal surface absent".into()))?;
        self.driver
            .draw(target.target, frame.clone())
            .map_err(error)?;
        target.frame = Some(frame.clone());
        Ok(())
    }
    pub fn close(&mut self, unit: Unit) -> Result<(), Error> {
        let target = self
            .targets
            .remove(&unit)
            .ok_or_else(|| Error("Metal surface absent".into()))?;
        self.receipts.retain(|receipt| receipt.unit != unit);
        self.visibility.retain(|receipt| receipt.unit != unit);
        self.recoveries.retain(|recovery| recovery.unit != unit);
        self.fallbacks.retain(|fallback| fallback.unit != unit);
        self.tracked.remove(&unit);
        self.awaiting.retain(|(pending, _)| *pending != unit);
        self.driver.close(target.target).map_err(error)
    }
    pub fn poll(&mut self) {
        for receipt in self.driver.poll(std::time::Instant::now()) {
            let Some((unit, _)) = self
                .targets
                .iter()
                .find(|(_, target)| target.target == receipt.target)
            else {
                continue;
            };
            let unit = *unit;
            if self.tracked.get(&unit) != Some(&receipt.frame) {
                if let Err(fault) = receipt.result {
                    eprintln!("[renderer:metal] {fault}");
                }
                continue;
            }
            self.tracked.remove(&unit);
            if let Err(fault) = &receipt.result {
                let frame = self
                    .targets
                    .get(&unit)
                    .and_then(|target| target.frame.clone())
                    .filter(|frame| frame.id() == receipt.frame);
                if let Some(frame) = frame {
                    self.schedule(unit, frame, receipt.drops, error(fault));
                    continue;
                }
                self.receipts.push(Receipt {
                    unit,
                    frame: receipt.frame,
                    produced: receipt.produced,
                    visible: receipt.visible,
                    drops: receipt.drops,
                    result: Err(error(fault)),
                });
                continue;
            }
            if receipt.visible.is_none() && receipt.result.is_ok() {
                self.awaiting.insert((unit, receipt.frame));
            }
            self.receipts.push(Receipt {
                unit,
                frame: receipt.frame,
                produced: receipt.produced,
                visible: receipt.visible,
                drops: receipt.drops,
                result: receipt.result.map_err(error),
            });
        }
        self.fallbacks();
        for receipt in self.driver.visible() {
            let Some((unit, _)) = self
                .targets
                .iter()
                .find(|(_, target)| target.target == receipt.target)
            else {
                continue;
            };
            if !self.awaiting.remove(&(*unit, receipt.frame)) {
                continue;
            }
            let Some(visible) = receipt.visible else {
                continue;
            };
            self.visibility.push(Visibility {
                unit: *unit,
                frame: receipt.frame,
                visible,
                drops: receipt.drops,
            });
        }
    }
    fn schedule(&mut self, unit: Unit, frame: Frame, drops: u64, fault: Error) {
        let result = frame
            .prepare(self.targets[&unit].output)
            .map_err(error)
            .and_then(|pass| self.compute.submit(pass).map_err(error));
        match result {
            Ok(task) => self.fallbacks.push(Fallback {
                unit,
                frame: frame.id(),
                fault,
                task,
            }),
            Err(recovery) => self.receipts.push(Receipt {
                unit,
                frame: frame.id(),
                produced: None,
                visible: None,
                drops,
                result: Err(Error(format!(
                    "Metal rendering failed ({fault}); CPU recovery failed ({recovery})"
                ))),
            }),
        }
    }
    fn fallbacks(&mut self) {
        let mut pending = Vec::new();
        for mut fallback in std::mem::take(&mut self.fallbacks) {
            match fallback.task.take() {
                Ok(Some(image)) => self.recoveries.push(Recovery {
                    unit: fallback.unit,
                    frame: fallback.frame,
                    image,
                    fault: fallback.fault,
                }),
                Ok(None) => pending.push(fallback),
                Err(recovery) => self.receipts.push(Receipt {
                    unit: fallback.unit,
                    frame: fallback.frame,
                    produced: None,
                    visible: None,
                    drops: 0,
                    result: Err(Error(format!(
                        "Metal rendering failed ({}); CPU recovery failed ({recovery})",
                        fallback.fault
                    ))),
                }),
            }
        }
        self.fallbacks = pending;
    }
    pub fn completed(&mut self) -> Vec<Receipt<Error>> {
        std::mem::take(&mut self.receipts)
    }
    pub fn visible(&mut self) -> Vec<Visibility> {
        std::mem::take(&mut self.visibility)
    }
    pub fn recoveries(&mut self) -> Vec<Recovery> {
        std::mem::take(&mut self.recoveries)
    }
}
