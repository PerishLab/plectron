use super::super::{Error, error};
use super::Recovery;
use crate::shared::{Receipt, Unit, Visibility};
use graphics::d3d12::Probe;
use graphics::shared::{Compute, Frame, Output, Task};
use std::collections::BTreeMap;
use std::time::Instant;

struct Job {
    unit: Unit,
    frame: u64,
    fault: Option<Error>,
    task: Task,
}

struct Watch {
    unit: Unit,
    frame: u64,
    baseline: u64,
    produced: Instant,
}

pub(super) struct Hybrid {
    probe: Probe,
    jobs: Vec<Job>,
    watches: Vec<Watch>,
    #[cfg(test)]
    hold: bool,
    #[cfg(test)]
    count: usize,
}

impl Hybrid {
    pub fn new(probe: Probe) -> Self {
        Self {
            probe,
            jobs: Vec::new(),
            watches: Vec::new(),
            #[cfg(test)]
            hold: false,
            #[cfg(test)]
            count: 0,
        }
    }

    pub fn ready(&self) -> bool {
        #[cfg(test)]
        if self.hold {
            return false;
        }
        self.probe.ready()
    }

    pub fn hedge(
        &mut self,
        unit: Unit,
        target: (&Frame, Output),
        compute: &Compute,
    ) -> Result<(), Error> {
        self.cancel(unit);
        let (frame, output) = target;
        let task = frame
            .prepare(output)
            .and_then(|pass| compute.submit(pass))
            .map_err(error)?;
        self.jobs.push(Job {
            unit,
            frame: frame.id(),
            fault: None,
            task,
        });
        #[cfg(test)]
        {
            self.count += 1;
        }
        Ok(())
    }

    pub fn fallback(
        &mut self,
        target: (Unit, Output),
        frame: Frame,
        failure: (u64, Error),
        compute: &Compute,
    ) -> Option<Receipt<Error>> {
        let (unit, output) = target;
        let (drops, fault) = failure;
        let result = frame
            .prepare(output)
            .map_err(error)
            .and_then(|pass| compute.submit(pass).map_err(error));
        match result {
            Ok(task) => {
                self.jobs.push(Job {
                    unit,
                    frame: frame.id(),
                    fault: Some(fault),
                    task,
                });
                None
            }
            Err(recovery) => Some(Receipt {
                unit,
                frame: frame.id(),
                produced: None,
                visible: None,
                drops,
                result: Err(Error(format!(
                    "D3D12 rendering failed ({fault}); CPU recovery failed ({recovery})"
                ))),
            }),
        }
    }

    pub fn cancel(&mut self, unit: Unit) {
        self.jobs
            .retain(|job| job.unit != unit || job.fault.is_some());
    }

    pub fn close(&mut self, unit: Unit) {
        self.jobs.retain(|job| job.unit != unit);
        self.watches.retain(|watch| watch.unit != unit);
    }

    pub fn watch(&mut self, unit: Unit, frame: u64, produced: Instant) {
        let Some(baseline) = graphics::d3d12::refresh() else {
            return;
        };
        self.watches.push(Watch {
            unit,
            frame,
            baseline,
            produced,
        });
    }

    pub fn awaiting(&self) -> bool {
        !self.watches.is_empty()
    }

    pub fn poll(
        &mut self,
        tracked: &mut BTreeMap<Unit, u64>,
        recoveries: &mut Vec<Recovery>,
        receipts: &mut Vec<Receipt<Error>>,
    ) {
        let mut pending = Vec::new();
        for mut job in std::mem::take(&mut self.jobs) {
            if job.fault.is_none() && tracked.get(&job.unit) != Some(&job.frame) {
                continue;
            }
            match job.task.take() {
                Ok(Some(image)) => complete(job, image, tracked, recoveries),
                Ok(None) => pending.push(job),
                Err(recovery) => fail(job, recovery, tracked, receipts),
            }
        }
        self.jobs = pending;
    }

    pub fn sightings(&mut self, visibility: &mut Vec<Visibility>) {
        let mut pending = Vec::new();
        for watch in std::mem::take(&mut self.watches) {
            match graphics::d3d12::settled(watch.baseline, watch.produced) {
                Some(visible) => visibility.push(Visibility {
                    unit: watch.unit,
                    frame: watch.frame,
                    visible,
                    drops: 0,
                }),
                None => pending.push(watch),
            }
        }
        self.watches = pending;
    }

    #[cfg(test)]
    pub fn hold(&mut self, enabled: bool) {
        self.hold = enabled;
    }

    #[cfg(test)]
    pub fn hedged(&self) -> usize {
        self.count
    }
}

fn complete(
    job: Job,
    image: graphics::shared::Image,
    tracked: &mut BTreeMap<Unit, u64>,
    recoveries: &mut Vec<Recovery>,
) {
    if job.fault.is_none() {
        tracked.remove(&job.unit);
    }
    recoveries.push(Recovery {
        unit: job.unit,
        frame: job.frame,
        image,
        fault: job.fault,
    });
}

fn fail(
    job: Job,
    recovery: graphics::shared::Fault,
    tracked: &mut BTreeMap<Unit, u64>,
    receipts: &mut Vec<Receipt<Error>>,
) {
    if job.fault.is_none() {
        tracked.remove(&job.unit);
    }
    let message = match job.fault {
        Some(fault) => {
            format!("D3D12 rendering failed ({fault}); CPU recovery failed ({recovery})")
        }
        None => recovery.to_string(),
    };
    receipts.push(Receipt {
        unit: job.unit,
        frame: job.frame,
        produced: None,
        visible: None,
        drops: 0,
        result: Err(Error(message)),
    });
}
