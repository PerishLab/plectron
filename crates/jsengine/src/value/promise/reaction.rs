use super::{Job, Jobs, Outcome, Reaction};
use crate::{Addons, Engine, Failure, Invocation, Value};

impl Engine {
    pub fn rejections(&self) -> Vec<Failure> {
        self.promises
            .entries
            .values()
            .filter(|entry| !entry.handled)
            .filter_map(|entry| {
                entry
                    .outcome
                    .as_ref()
                    .and_then(|outcome| outcome.as_ref().err())
                    .cloned()
            })
            .collect()
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
                .rejections()
                .iter()
                .map(|failure| self.diagnostic(failure).0)
                .collect(),
        }
    }

    pub fn checkpoint(&mut self, host: &mut impl Addons) -> Result<usize, Failure> {
        if self.promises.draining {
            return Ok(0);
        }
        self.promises.draining = true;
        self.enter();
        let result = self.drain(host);
        self.turns -= 1;
        self.promises.draining = false;
        result
    }
    fn drain(&mut self, host: &mut impl Addons) -> Result<usize, Failure> {
        let mut count = 0;
        while let Some(job) = self.promises.queue.pop_front() {
            let (job, mut scope) = job.receive("js.job");
            self.tick()?;
            let result = self.job(job, host);
            scope.finish(result.is_ok());
            result?;
            count += 1;
            self.promises.completed += 1;
        }
        Ok(count)
    }
    fn job(&mut self, job: Job, host: &mut impl Addons) -> Result<(), Failure> {
        match job {
            Job::Assimilate {
                target,
                source,
                then,
            } => self.assimilate((target, source, then), host),
            Job::React(reaction, outcome) => self.reaction(reaction, outcome, host),
        }
    }
    fn reaction(
        &mut self,
        reaction: Reaction,
        outcome: Outcome,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        match reaction {
            Reaction::Resume(task) => self.resume(*task, outcome, host),
            Reaction::Forward { target, preserved } => {
                let result = if outcome.is_ok() {
                    preserved.unwrap_or(outcome)
                } else {
                    outcome
                };
                self.finish(&target, result, host)
            }
            Reaction::Then { target, yes, no } => {
                let callback = if outcome.is_ok() { yes } else { no };
                self.then((target, callback), outcome, host)
            }
            Reaction::Finally { target, callback } => {
                self.cleanup((target, callback, outcome), host)
            }
        }
    }
    fn then(
        &mut self,
        input: (Value, Option<Value>),
        outcome: Outcome,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let result = match input.1 {
            Some(callback) => self.callback(callback, outcome, host),
            None => outcome,
        };
        self.finish(&input.0, result, host)
    }

    fn callback(
        &mut self,
        callback: Value,
        outcome: Outcome,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        if let Err(failure) = &outcome
            && self.rejecting(&callback)
        {
            self.rejected(&callback, failure.clone())?;
            return Ok(Value::Undefined);
        }
        self.apply(callback, Invocation::direct(vec![argument(outcome)]), host)
    }
    pub(crate) fn finish(
        &mut self,
        target: &Value,
        result: Result<Value, Failure>,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        match result {
            Ok(value) => self.adopt(target, value, host),
            Err(failure) if failure.thrown().is_some() => Ok(self.fulfill(target, Err(failure))?),
            Err(failure) => Err(failure),
        }
    }
}

fn argument(outcome: Outcome) -> Value {
    match outcome {
        Ok(value) => value,
        Err(failure) => failure.thrown().expect("promise rejection").clone(),
    }
}

impl Engine {
    fn assimilate(
        &mut self,
        input: (Value, Value, Value),
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let (target, source, then) = input;
        let (resolve, reject) = self.resolvers(target);
        let result = self.apply(
            then,
            Invocation {
                receiver: source,
                arguments: vec![resolve, reject.clone()],
            },
            host,
        );
        if let Err(failure) = result {
            if failure.thrown().is_some() {
                self.rejected(&reject, failure)?;
            } else {
                return Err(failure);
            }
        }
        Ok(())
    }
    fn cleanup(
        &mut self,
        input: (Value, Value, Outcome),
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let (target, callback, outcome) = input;
        match self.apply(callback, Invocation::direct(Vec::new()), host) {
            Ok(value) => {
                let awaited = self.resolved(value, host)?;
                Ok(self.subscribe(
                    &awaited,
                    Reaction::Forward {
                        target,
                        preserved: Some(outcome),
                    },
                )?)
            }
            Err(failure) => self.finish(&target, Err(failure), host),
        }
    }
}
