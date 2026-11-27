use super::super::{Error, error, window::Window};
use super::Recovery;
use super::hybrid::Hybrid;
use crate::shared::{Receipt, Unit, Visibility};
use graphics::d3d12::{Executor, Surface};
use graphics::shared::{Compute, Driver, Frame, Output, Target};
use std::collections::{BTreeMap, BTreeSet};

pub struct Accelerator {
    targets: BTreeMap<Unit, Destination>,
    driver: Driver<Executor, Surface>,
    receipts: Vec<Receipt<Error>>,
    visibility: Vec<Visibility>,
    recoveries: Vec<Recovery>,
    hybrid: Hybrid,
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

impl Accelerator {
    pub fn new(notify: shared::Notify) -> Result<Self, Error> {
        let executor = Executor::new(notify.clone()).map_err(error)?;
        let probe = executor.probe();
        let compute = Compute::new(notify.clone()).map_err(error)?;
        Ok(Self {
            targets: BTreeMap::new(),
            driver: Driver::new(executor),
            receipts: Vec::new(),
            visibility: Vec::new(),
            recoveries: Vec::new(),
            hybrid: Hybrid::new(probe),
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
        let result = window.surface(target, output).and_then(|surface| {
            self.driver
                .attach(surface.notified(self.notify.clone()))
                .map_err(error)
        });
        if let Err(fault) = result {
            let _ = self.driver.close(target);
            return Err(fault);
        }
        Ok(target)
    }

    fn rebind(&mut self, unit: Unit, window: &Window, output: Output) -> Result<(), Error> {
        let current = self
            .targets
            .get(&unit)
            .ok_or_else(|| Error("D3D12 surface absent".into()))?;
        if current.output == output {
            return Ok(());
        }
        let old = current.target;
        self.driver.close(old).map_err(error)?;
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

    pub fn refresh(&mut self, unit: Unit, window: &Window) -> Result<(), Error> {
        let Some(frame) = self.targets.get(&unit).and_then(|dest| dest.frame.clone()) else {
            return Ok(());
        };
        let output = Output::new(frame.extent(), window.scale()).map_err(error)?;
        if self
            .targets
            .get(&unit)
            .is_none_or(|dest| dest.output == output)
        {
            return Ok(());
        }
        self.rebind(unit, window, output)?;
        self.paint(unit, window, &frame)
    }

    pub fn draw(&mut self, unit: Unit, window: &Window, frame: &Frame) -> Result<(), Error> {
        if !self.hybrid.ready() {
            let output = Output::new(frame.extent(), window.scale()).map_err(error)?;
            self.rebind(unit, window, output)?;
            let target = self
                .targets
                .get_mut(&unit)
                .ok_or_else(|| Error("D3D12 surface absent".into()))?;
            self.hybrid.hedge(unit, (frame, output), &self.compute)?;
            target.frame = Some(frame.clone());
            self.tracked.insert(unit, frame.id());
            return Ok(());
        }
        self.hybrid.cancel(unit);
        self.paint(unit, window, frame)?;
        self.tracked.insert(unit, frame.id());
        Ok(())
    }

    pub fn paint(&mut self, unit: Unit, window: &Window, frame: &Frame) -> Result<(), Error> {
        let output = Output::new(frame.extent(), window.scale()).map_err(error)?;
        self.rebind(unit, window, output)?;
        let target = self
            .targets
            .get_mut(&unit)
            .ok_or_else(|| Error("D3D12 surface absent".into()))?;
        self.driver
            .draw(target.target, frame.clone())
            .map_err(error)?;
        target.frame = Some(frame.clone());
        Ok(())
    }

    pub fn close(&mut self, unit: Unit) -> Result<(), Error> {
        let Some(target) = self.targets.remove(&unit) else {
            return Ok(());
        };
        self.receipts.retain(|receipt| receipt.unit != unit);
        self.visibility.retain(|receipt| receipt.unit != unit);
        self.recoveries.retain(|recovery| recovery.unit != unit);
        self.hybrid.close(unit);
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
                    eprintln!("[renderer:d3d12] {fault}");
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
        self.hybrid
            .poll(&mut self.tracked, &mut self.recoveries, &mut self.receipts);
        self.hybrid.sightings(&mut self.visibility);
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
        let output = self.targets[&unit].output;
        if let Some(receipt) =
            self.hybrid
                .fallback((unit, output), frame, (drops, fault), &self.compute)
        {
            self.receipts.push(receipt);
        }
    }

    pub fn watch(&mut self, unit: Unit, frame: u64, produced: std::time::Instant) {
        self.hybrid.watch(unit, frame, produced);
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

    pub fn awaiting(&self) -> bool {
        !self.awaiting.is_empty() || self.hybrid.awaiting()
    }

    pub fn compute(&self) -> &Compute {
        &self.compute
    }

    #[cfg(test)]
    pub(super) fn inject(&mut self, unit: Unit, frame: Frame, fault: Error) {
        self.schedule(unit, frame, 0, fault);
    }

    #[cfg(test)]
    pub(super) fn hold(&mut self, enabled: bool) {
        self.hybrid.hold(enabled);
    }

    #[cfg(test)]
    pub(super) fn hedged(&self) -> usize {
        self.hybrid.hedged()
    }

    #[cfg(test)]
    pub(super) fn ready(&self) -> bool {
        self.hybrid.ready()
    }
}
