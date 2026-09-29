mod builtin;
mod reaction;
mod resolve;
use crate::Handle;
use crate::{Engine, Fault, Value};
pub(crate) use builtin::Method;
use std::collections::{BTreeMap, HashSet, VecDeque};
pub(super) type Outcome = Result<Value, Value>;

#[derive(Clone)]
pub struct Deferred {
    pub promise: Value,
    pub resolve: Value,
    pub reject: Value,
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Jobs {
    pub queued: usize,
    pub pending: usize,
    pub completed: usize,
    pub rejected: Vec<String>,
}

#[derive(Default)]
pub(crate) struct State {
    prototype: Option<Value>,
    entries: BTreeMap<usize, Entry>,
    resolvers: Vec<Option<(Value, bool)>>,
    vacant: Vec<usize>,
    queue: VecDeque<observe::Envelope<Job>>,
    completed: usize,
    draining: bool,
}
struct Entry {
    outcome: Option<Outcome>,
    reactions: Vec<Reaction>,
    handled: bool,
}

impl State {
    pub(crate) fn slots(&self) -> usize {
        self.resolvers.len()
    }

    pub(crate) fn prune(&mut self, dead: &[Handle], reachable: &HashSet<usize>) {
        let dead: HashSet<_> = dead.iter().copied().collect();
        for handle in &dead {
            self.entries.remove(&handle.slot);
        }
        for (id, resolver) in self.resolvers.iter_mut().enumerate() {
            let Some((promise, used)) = resolver else {
                continue;
            };
            if !reachable.contains(&id) {
                *resolver = None;
                self.vacant.push(id);
            } else if let Value::Object(handle) = promise
                && dead.contains(handle)
            {
                *promise = Value::Undefined;
                *used = true;
            }
        }
    }
    pub(crate) fn ready(&self) -> bool {
        !self.draining
    }

    pub(crate) fn roots(
        &self,
        value: &mut dyn FnMut(&Value),
        scope: &mut dyn FnMut(usize),
        binding: &mut dyn FnMut(usize),
    ) {
        if let Some(prototype) = &self.prototype {
            value(prototype);
        }
        for job in &self.queue {
            job.peek().trace(value, scope, binding);
        }
    }

    pub(crate) fn entry(
        &self,
        slot: usize,
        value: &mut dyn FnMut(&Value),
        scope: &mut dyn FnMut(usize),
        binding: &mut dyn FnMut(usize),
    ) {
        if let Some(entry) = self.entries.get(&slot) {
            if let Some(outcome) = &entry.outcome {
                result(outcome, value);
            }
            for reaction in &entry.reactions {
                reaction.trace(value, scope, binding);
            }
        }
    }

    pub(crate) fn resolver(&self, id: usize, value: &mut dyn FnMut(&Value)) {
        if let Some(Some((promise, false))) = self.resolvers.get(id) {
            value(promise);
        }
    }
}

fn result(outcome: &Outcome, value: &mut dyn FnMut(&Value)) {
    match outcome {
        Ok(item) | Err(item) => value(item),
    }
}
enum Reaction {
    Then {
        target: Value,
        yes: Option<Value>,
        no: Option<Value>,
    },
    Finally {
        target: Value,
        callback: Value,
    },
    Forward {
        target: Value,
        preserved: Option<Outcome>,
    },
    Resume(Box<crate::eval::flow::Task>),
}
enum Job {
    React(Reaction, Outcome),
    Assimilate {
        target: Value,
        source: Value,
        then: Value,
    },
}

impl Reaction {
    fn trace(
        &self,
        value: &mut dyn FnMut(&Value),
        scope: &mut dyn FnMut(usize),
        binding: &mut dyn FnMut(usize),
    ) {
        match self {
            Self::Then { target, yes, no } => {
                value(target);
                if let Some(yes) = yes {
                    value(yes);
                }
                if let Some(no) = no {
                    value(no);
                }
            }
            Self::Finally { target, callback } => {
                value(target);
                value(callback);
            }
            Self::Forward { target, preserved } => {
                value(target);
                if let Some(outcome) = preserved {
                    result(outcome, value);
                }
            }
            Self::Resume(task) => task.trace(value, scope, binding),
        }
    }
}

impl Job {
    fn trace(
        &self,
        value: &mut dyn FnMut(&Value),
        scope: &mut dyn FnMut(usize),
        binding: &mut dyn FnMut(usize),
    ) {
        match self {
            Self::React(reaction, outcome) => {
                reaction.trace(value, scope, binding);
                result(outcome, value);
            }
            Self::Assimilate {
                target,
                source,
                then,
            } => {
                value(target);
                value(source);
                value(then);
            }
        }
    }
}

impl Engine {
    pub fn deferred(&mut self) -> Result<Deferred, Fault> {
        let promise = self.promise()?;
        let (resolve, reject) = self.resolvers(promise.clone());
        Ok(Deferred {
            promise,
            resolve,
            reject,
        })
    }
    pub(crate) fn promise(&mut self) -> Result<Value, Fault> {
        if self.promises.entries.len() >= 16_384 {
            return Err(Fault("promise budget exhausted".into()));
        }
        let value = self.object(Default::default())?;
        let prototype = self.promises.prototype.clone();
        self.inherit(&value, prototype.as_ref());
        let Value::Object(handle) = &value else {
            unreachable!()
        };
        self.promises.entries.insert(
            handle.slot,
            Entry {
                outcome: None,
                reactions: Vec::new(),
                handled: false,
            },
        );
        Ok(value)
    }
    fn resolvers(&mut self, promise: Value) -> (Value, Value) {
        let id = if let Some(id) = self.promises.vacant.pop() {
            self.promises.resolvers[id] = Some((promise, false));
            id
        } else {
            let id = self.promises.resolvers.len();
            self.promises.resolvers.push(Some((promise, false)));
            id
        };
        (
            self.function(super::Call::Resolve(id, false)),
            self.function(super::Call::Resolve(id, true)),
        )
    }
    fn slot(&self, value: &Value) -> Result<usize, Fault> {
        self.validate(value)?;
        match value {
            Value::Object(handle) if self.promises.entries.contains_key(&handle.slot) => {
                Ok(handle.slot)
            }
            _ => Err(Fault("expected an engine-owned promise".into())),
        }
    }
    pub fn jobs(&self) -> Jobs {
        Jobs {
            queued: self.promises.queue.len(),
            pending: self
                .promises
                .entries
                .values()
                .filter(|entry| entry.outcome.is_none())
                .count(),
            completed: self.promises.completed,
            rejected: self
                .promises
                .entries
                .values()
                .filter(|entry| !entry.handled)
                .filter_map(|entry| match &entry.outcome {
                    Some(Err(value)) => {
                        Some(self.diagnostic(&crate::Failure::Throw(value.clone())).0)
                    }
                    _ => None,
                })
                .collect(),
        }
    }
    fn enqueue(&mut self, job: Job) -> Result<(), Fault> {
        if self.promises.queue.len() >= 16_384 {
            return Err(Fault("job queue budget exhausted".into()));
        }
        self.promises.queue.push_back(job.into());
        Ok(())
    }
    fn subscribe(&mut self, promise: &Value, reaction: Reaction) -> Result<(), Fault> {
        let slot = self.slot(promise)?;
        let entry = self.promises.entries.get_mut(&slot).expect("promise slot");
        entry.handled = true;
        if let Some(outcome) = entry.outcome.clone() {
            self.enqueue(Job::React(reaction, outcome))
        } else {
            if entry.reactions.len() >= 16_384 {
                return Err(Fault("reaction budget exhausted".into()));
            }
            entry.reactions.push(reaction);
            Ok(())
        }
    }
}
