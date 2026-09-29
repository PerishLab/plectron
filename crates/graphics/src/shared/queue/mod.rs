mod ticket;
use crate::shared::{Extent, Fault, Frame, Image, Output, Pass};
use pipeline::Graph;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
pub use ticket::{State, Submission, Target};

static OWNER: AtomicU64 = AtomicU64::new(1);

pub trait Backend {
    type Work;
    fn submit(&mut self, pass: &Pass) -> Result<Self::Work, Fault>;
    fn render(&mut self, frame: &Frame) -> Result<Self::Work, Fault> {
        self.submit(&frame.prepare(Output::new(frame.extent(), 1.0)?)?)
    }
    fn poll(&mut self, work: &mut Self::Work) -> Result<State, Fault>;
    fn read(&mut self, work: &Self::Work) -> Result<Image, Fault>;
}

pub trait Sink<W> {
    fn target(&self) -> Target;
    fn present(&mut self, work: &W) -> Result<(), Fault>;
    fn poll(&mut self) -> Result<State, Fault> {
        Ok(State::Complete)
    }
    fn completed(&self) -> Option<std::time::Instant> {
        None
    }
    fn visibility(&self) -> crate::shared::Visibility {
        crate::shared::Visibility::Unobserved
    }
    fn cancel(&mut self) {}
}

struct Entry<W> {
    target: Target,
    work: W,
    state: State,
    retired: bool,
}

pub struct Queue<B: Backend> {
    preparation: crate::shared::Preparation,
    backend: B,
    owner: u64,
    sequence: u64,
    targets: BTreeMap<u64, Output>,
    entries: BTreeMap<u64, Entry<B::Work>>,
    fault: Option<Fault>,
}

impl<B: Backend> Queue<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            preparation: crate::shared::Preparation::default(),
            owner: OWNER.fetch_add(1, Ordering::Relaxed),
            sequence: 0,
            targets: BTreeMap::new(),
            entries: BTreeMap::new(),
            fault: None,
        }
    }

    pub fn preparation(&mut self) -> &mut crate::shared::Preparation {
        &mut self.preparation
    }
    pub fn target(&mut self, extent: Extent) -> Result<Target, Fault> {
        self.output(Output::new(extent, 1.0)?)
    }
    pub fn output(&mut self, output: Output) -> Result<Target, Fault> {
        self.healthy()?;
        if self.targets.len() >= 8 {
            return Err(Fault("target budget exhausted".into()));
        }
        let slot = self.next()?;
        self.targets.insert(slot, output);
        Ok(Target {
            owner: self.owner,
            slot,
        })
    }

    pub(crate) fn validate(&self, target: Target) -> Result<(), Fault> {
        self.extent(target).map(|_| ())
    }
    pub(crate) fn matches(&self, target: Target, extent: Extent) -> Result<(), Fault> {
        if self.extent(target)? != extent {
            return Err(Fault("frame exceeds target extent".into()));
        }
        Ok(())
    }

    pub fn close(&mut self, target: Target) -> Result<(), Fault> {
        self.extent(target)?;
        self.targets.remove(&target.slot);
        Ok(())
    }

    pub fn submit(&mut self, target: Target, frame: &Frame) -> Result<Submission, Fault> {
        self.healthy()?;
        if self.extent(target)? != frame.extent() {
            return Err(Fault("frame exceeds target extent".into()));
        }
        if !self.available() {
            return Err(Fault("submission queue is full".into()));
        }
        let slot = self.next()?;
        let output = self.descriptor(target)?;
        let work = crate::shared::operation::submission(&mut self.preparation, &mut self.backend)
            .run((frame, output))?;
        self.entries.insert(
            slot,
            Entry {
                target,
                work,
                state: State::Pending,
                retired: false,
            },
        );
        Ok(Submission {
            owner: self.owner,
            slot,
        })
    }

    pub fn available(&self) -> bool {
        self.fault.is_none()
            && self.entries.len() < 8
            && self
                .entries
                .values()
                .filter(|entry| entry.state == State::Pending)
                .count()
                < 2
    }

    pub fn poll(&mut self) -> Result<(), Fault> {
        self.healthy()?;
        for entry in self.entries.values_mut() {
            if entry.state != State::Pending {
                continue;
            }
            match pipeline::step(crate::shared::operation::Completion {
                backend: &mut self.backend,
            })
            .run(&mut entry.work)
            {
                Ok(state) => entry.state = state,
                Err(fault) => {
                    self.fault = Some(fault);
                    break;
                }
            }
        }
        if let Some(fault) = &self.fault {
            for entry in self.entries.values_mut() {
                if entry.state == State::Pending {
                    entry.state = State::Failed(fault.clone());
                }
            }
        }
        self.entries
            .retain(|_, entry| !entry.retired || entry.state == State::Pending);
        self.healthy()
    }

    pub fn wait(
        &mut self,
        submission: Submission,
        timeout: std::time::Duration,
    ) -> Result<(), Fault> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            self.poll()?;
            match self.state(submission)? {
                State::Complete => return Ok(()),
                State::Failed(fault) => return Err(fault),
                State::Pending => (),
            }
            if std::time::Instant::now() >= deadline {
                return Err(Fault("submission wait timed out".into()));
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    pub fn state(&self, submission: Submission) -> Result<State, Fault> {
        Ok(self.entry(submission)?.state.clone())
    }

    pub fn read(&mut self, submission: Submission) -> Result<Image, Fault> {
        self.complete(submission)?;
        self.backend.read(&self.entries[&submission.slot].work)
    }

    pub fn release(&mut self, submission: Submission) -> Result<(), Fault> {
        self.entry(submission)?;
        if self.entries[&submission.slot].state == State::Pending {
            self.entries
                .get_mut(&submission.slot)
                .expect("checked submission")
                .retired = true;
        } else {
            self.entries.remove(&submission.slot);
        }
        Ok(())
    }

    pub fn retained(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn extent(&self, target: Target) -> Result<Extent, Fault> {
        Ok(self.descriptor(target)?.logical())
    }
    pub fn descriptor(&self, target: Target) -> Result<Output, Fault> {
        if target.owner != self.owner {
            return Err(Fault("foreign target".into()));
        }
        self.targets
            .get(&target.slot)
            .copied()
            .ok_or_else(|| Fault("target is closed or unknown".into()))
    }

    pub fn present(
        &self,
        submission: Submission,
        sink: &mut impl Sink<B::Work>,
    ) -> Result<(), Fault> {
        self.complete(submission)?;
        let entry = self.entry(submission)?;
        self.extent(entry.target)?;
        if sink.target() != entry.target {
            return Err(Fault("submission belongs to another target".into()));
        }
        sink.present(&entry.work)
    }

    pub fn destination(&self, submission: Submission) -> Result<Target, Fault> {
        Ok(self.entry(submission)?.target)
    }

    fn complete(&self, submission: Submission) -> Result<(), Fault> {
        match &self.entry(submission)?.state {
            State::Complete => Ok(()),
            State::Pending => Err(Fault("submission is pending".into())),
            State::Failed(fault) => Err(fault.clone()),
        }
    }

    fn entry(&self, submission: Submission) -> Result<&Entry<B::Work>, Fault> {
        if submission.owner != self.owner {
            return Err(Fault("foreign submission".into()));
        }
        self.entries
            .get(&submission.slot)
            .filter(|entry| !entry.retired)
            .ok_or_else(|| Fault("submission is released or unknown".into()))
    }

    fn healthy(&self) -> Result<(), Fault> {
        self.fault.clone().map_or(Ok(()), Err)
    }

    fn next(&mut self) -> Result<u64, Fault> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Fault("render identifiers exhausted".into()))?;
        Ok(self.sequence)
    }
}
