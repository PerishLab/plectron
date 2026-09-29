use super::{
    Data,
    record::{Action, Outcome, Record},
};
use crate::Fault;
use domengine::Node;
use std::collections::VecDeque;

#[derive(Clone, Copy)]
pub enum Control {
    Stop,
    Immediate,
    Cancel,
}
#[derive(Clone, Copy)]
pub struct Lease {
    pub event: u64,
    pub call: u64,
}
pub struct Frame {
    pub record: Record,
    pub stopped: bool,
    pub immediate: bool,
    pub cancelable: bool,
    pub current: Option<(u64, u64)>,
}
#[derive(Default)]
pub struct Dispatches {
    next: u64,
    stack: Vec<Frame>,
    records: VecDeque<Record>,
    pub job: Option<u64>,
}
impl Dispatches {
    fn identifier(&mut self) -> Result<u64, Fault> {
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| Fault("dispatch IDs exhausted".into()))?;
        Ok(self.next)
    }
    pub fn begin(&mut self, path: &[Node], data: &Data) -> Result<u64, Fault> {
        if self.stack.len() >= 32 {
            return Err(Fault("event nesting budget exhausted".into()));
        }
        let id = self.identifier()?;
        let parent = self.stack.last().map(|frame| frame.record.id);
        self.stack.push(Frame {
            record: Record {
                id,
                parent,
                job: self.job,
                target: path[0].id(),
                kind: data.kind.clone(),
                input: data.input.map(String::from),
                origin: data.origin,
                path: path.iter().map(|node| node.id()).collect(),
                deliveries: Vec::new(),
                omitted: 0,
                errors: 0,
                exception: None,
                cancelled: None,
                stopped: None,
                immediate: false,
                failure: None,
                action: Action::Notification,
            },
            stopped: false,
            immediate: false,
            cancelable: data.cancelable,
            current: None,
        });
        Ok(id)
    }
    pub fn frame(&mut self) -> &mut Frame {
        self.stack.last_mut().expect("active dispatch")
    }
    pub fn enter(&mut self, subscription: u64) -> Result<Lease, Fault> {
        let call = self.identifier()?;
        let frame = self.frame();
        frame.current = Some((call, subscription));
        Ok(Lease {
            event: frame.record.id,
            call,
        })
    }
    pub fn control(&mut self, lease: Lease, control: Control) -> bool {
        let Some(frame) = self.stack.last_mut() else {
            return false;
        };
        let Some((call, subscription)) = frame.current else {
            return false;
        };
        if lease.event != frame.record.id || lease.call != call {
            return false;
        }
        match control {
            Control::Cancel if frame.cancelable => {
                frame.record.cancelled.get_or_insert(subscription);
            }
            Control::Cancel => return false,
            Control::Stop | Control::Immediate => {
                frame.stopped = true;
                frame.record.stopped.get_or_insert(subscription);
                if matches!(control, Control::Immediate) {
                    frame.immediate = true;
                    frame.record.immediate = true;
                }
            }
        }
        true
    }
    pub fn finish(&mut self, failure: Option<String>) -> Outcome {
        let mut frame = self.stack.pop().expect("active dispatch");
        frame.record.failure = failure.map(|text| text.chars().take(256).collect());
        if frame.record.failure.is_some() {
            frame.record.action = Action::Failed;
        }
        let outcome = Outcome {
            id: frame.record.id,
            cancelled: frame.record.cancelled.is_some(),
        };
        self.records.push_back(frame.record);
        if self.records.len() > 32 {
            self.records.pop_front();
        }
        outcome
    }
    pub fn action(&mut self, id: u64, action: Action) {
        if let Some(record) = self.records.iter_mut().find(|record| record.id == id) {
            record.action = action;
        }
    }
    pub fn fail(&mut self, id: u64, fault: &Fault) {
        if let Some(record) = self.records.iter_mut().find(|record| record.id == id) {
            record.action = Action::Failed;
            record.failure = Some(fault.to_string().chars().take(256).collect());
        }
    }
    pub fn snapshot(&self) -> Vec<Record> {
        self.records.iter().cloned().collect()
    }
}
