use crate::shared::{Backend, Extent, Fault, Frame, Queue, Sink, State, Target};
use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};
mod state;
use state::{Flight, Update};
pub use state::{Pacing, Pulse, Receipt, Sighting, Visibility};

struct Slot<S> {
    sink: S,
    pending: Option<Frame>,
    flight: Option<Flight>,
    fault: Option<Fault>,
}

pub struct Driver<B: Backend, S> {
    queue: Queue<B>,
    slots: BTreeMap<Target, Slot<S>>,
    order: VecDeque<Target>,
    visibility: Vec<Sighting>,
    fault: Option<Fault>,
}

impl<B: Backend, S: Sink<B::Work>> Driver<B, S> {
    pub fn new(backend: B) -> Self {
        Self {
            queue: Queue::new(backend),
            slots: BTreeMap::new(),
            order: VecDeque::new(),
            visibility: Vec::new(),
            fault: None,
        }
    }
    pub fn target(&mut self, extent: Extent) -> Result<Target, Fault> {
        self.queue.target(extent)
    }
    pub fn output(&mut self, output: crate::shared::Output) -> Result<Target, Fault> {
        self.queue.output(output)
    }
    pub fn attach(&mut self, sink: S) -> Result<(), Fault> {
        let target = sink.target();
        if self.slots.contains_key(&target) {
            return Err(Fault("target already attached".into()));
        }
        self.queue.validate(target)?;
        self.slots.insert(
            target,
            Slot {
                sink,
                pending: None,
                flight: None,
                fault: None,
            },
        );
        self.order.push_back(target);
        Ok(())
    }
    pub fn draw(&mut self, target: Target, frame: Frame) -> Result<Option<u64>, Fault> {
        if let Some(fault) = &self.fault {
            return Err(fault.clone());
        }
        let slot = self
            .slots
            .get_mut(&target)
            .ok_or_else(|| Fault("target is absent".into()))?;
        if let Some(fault) = &slot.fault {
            return Err(fault.clone());
        }
        self.queue.matches(target, frame.extent())?;
        Ok(slot.pending.replace(frame).map(|frame| frame.id()))
    }
    pub fn close(&mut self, target: Target) -> Result<(), Fault> {
        self.queue.close(target)?;
        if let Some(slot) = self.slots.remove(&target)
            && let Some(flight) = slot.flight
        {
            self.queue.release(flight.ticket)?;
        }
        self.order.retain(|entry| *entry != target);
        Ok(())
    }
    pub fn poll(&mut self, now: Instant) -> Vec<Receipt> {
        if let Err(fault) = self.queue.poll() {
            self.fault = Some(fault);
        }
        let mut receipts = Vec::new();
        for (target, slot) in &mut self.slots {
            let Some(update) = slot.poll(*target, &self.queue, (self.fault.as_ref(), now)) else {
                continue;
            };
            if update.finished
                && let Some(flight) = slot.flight.take()
            {
                self.queue.release(flight.ticket).expect("owned submission");
            }
            if let Some(visible) = update.visible {
                self.visibility.push(visible);
            }
            if let Some(receipt) = update.receipt {
                slot.accept::<B::Work>(&receipt);
                receipts.push(receipt);
            }
        }
        for _ in 0..self.order.len() {
            let target = self.order.pop_front().expect("scheduled target");
            self.order.push_back(target);
            let slot = self.slots.get_mut(&target).expect("attached target");
            if slot.flight.is_some() {
                continue;
            }
            if let Some(fault) = slot.fault.as_ref().or(self.fault.as_ref()) {
                if let Some(frame) = slot.pending.take() {
                    receipts.push(Receipt {
                        target,
                        frame: frame.id(),
                        produced: None,
                        visible: None,
                        drops: 0,
                        result: Err(fault.clone()),
                    });
                }
                continue;
            }
            if !self.queue.available() {
                break;
            }
            if let Some(receipt) = slot.submit(target, &mut self.queue, now) {
                receipts.push(receipt);
            }
        }
        receipts
    }
    pub fn visible(&mut self) -> Vec<Sighting> {
        std::mem::take(&mut self.visibility)
    }
}

impl<S> Slot<S> {
    fn accept<W>(&mut self, receipt: &Receipt)
    where
        S: Sink<W>,
    {
        if let Err(fault) = &receipt.result {
            self.sink.cancel();
            self.fault = Some(fault.clone());
        }
    }

    fn submit<B: Backend>(
        &mut self,
        target: Target,
        queue: &mut Queue<B>,
        now: Instant,
    ) -> Option<Receipt> {
        let frame = self.pending.take()?;
        match queue.submit(target, &frame) {
            Ok(ticket) => {
                self.flight = Some(Flight::new(frame.id(), ticket, now));
                None
            }
            Err(fault) => {
                self.fault = Some(fault.clone());
                Some(Receipt {
                    target,
                    frame: frame.id(),
                    produced: None,
                    visible: None,
                    drops: 0,
                    result: Err(fault),
                })
            }
        }
    }
    fn poll<B: Backend>(
        &mut self,
        target: Target,
        queue: &Queue<B>,
        timing: (Option<&Fault>, Instant),
    ) -> Option<Update>
    where
        S: Sink<B::Work>,
    {
        let (fault, now) = timing;
        let flight = self.flight.as_mut()?;
        if let Some(fault) = fault {
            return Some(flight.terminal(target, Err(fault.clone())));
        }
        if now.saturating_duration_since(flight.started) > Duration::from_secs(10) {
            if flight.reported {
                return Some(Update {
                    receipt: None,
                    visible: Some(Sighting {
                        target,
                        frame: flight.frame,
                        visible: None,
                        drops: flight.drops,
                    }),
                    finished: true,
                });
            }
            return Some(
                flight.terminal(target, Err(Fault("presentation deadline exceeded".into()))),
            );
        }
        if flight.presenting {
            return self.presented::<B::Work>(target);
        }
        match queue.state(flight.ticket) {
            Ok(State::Pending) => None,
            Ok(State::Complete) => {
                flight.presenting = true;
                flight.complete = false;
                queue
                    .present(flight.ticket, &mut self.sink)
                    .err()
                    .map(|fault| {
                        let flight = self.flight.as_mut().expect("owned flight");
                        flight.terminal(target, Err(fault))
                    })
            }
            Ok(State::Failed(fault)) | Err(fault) => Some(flight.terminal(target, Err(fault))),
        }
    }

    fn presented<W>(&mut self, target: Target) -> Option<Update>
    where
        S: Sink<W>,
    {
        let flight = self.flight.as_mut().expect("owned flight");
        let state = if flight.complete {
            Ok(State::Complete)
        } else {
            self.sink.poll()
        };
        match state {
            Ok(State::Pending) => return None,
            Ok(State::Failed(fault)) | Err(fault) => {
                return Some(flight.terminal(target, Err(fault)));
            }
            Ok(State::Complete) => {}
        }
        flight.complete = true;
        flight.produced = flight.produced.or_else(|| self.sink.completed());
        match self.sink.visibility() {
            Visibility::Pending if flight.reported => None,
            Visibility::Pending => Some(flight.initial(target)),
            Visibility::Dropped => {
                flight.drops = flight.drops.saturating_add(1);
                flight.presenting = false;
                flight.complete = false;
                (!flight.reported).then(|| flight.initial(target))
            }
            Visibility::Visible(visible) if flight.reported => Some(Update {
                receipt: None,
                visible: Some(Sighting {
                    target,
                    frame: flight.frame,
                    visible: Some(visible),
                    drops: flight.drops,
                }),
                finished: true,
            }),
            Visibility::Visible(visible) => Some(flight.terminal(target, Ok(())).visible(visible)),
            Visibility::Unobserved => Some(flight.terminal(target, Ok(()))),
        }
    }
}
