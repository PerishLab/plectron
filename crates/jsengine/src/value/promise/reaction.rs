use super::{Job, Outcome, Reaction};
use crate::{Addons, Engine, Failure, Fault, Invocation, Value};

impl Engine {
    pub fn checkpoint(&mut self, host: &mut impl Addons) -> Result<usize, Fault> {
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
    fn drain(&mut self, host: &mut impl Addons) -> Result<usize, Fault> {
        let mut count = 0;
        while let Some(job) = self.promises.queue.pop_front() {
            let (job, mut scope) = job.receive("js.job");
            self.tick()?;
            let result = self.job(job, host);
            scope.finish(result.is_ok());
            result.map_err(|failure| self.diagnostic(&failure))?;
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
                self.finish(&target, result.map_err(Failure::Throw), host)
            }
            Reaction::Then { target, yes, no } => {
                let callback = if outcome.is_ok() { yes } else { no };
                let result = match callback {
                    Some(callback) => self.apply(
                        callback,
                        Invocation::direct(vec![outcome.unwrap_or_else(|value| value)]),
                        host,
                    ),
                    None => outcome.map_err(Failure::Throw),
                };
                self.finish(&target, result, host)
            }
            Reaction::Finally { target, callback } => {
                self.cleanup((target, callback, outcome), host)
            }
        }
    }
    pub(crate) fn finish(
        &mut self,
        target: &Value,
        result: Result<Value, Failure>,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        match result {
            Ok(value) => self.adopt(target, value, host),
            Err(Failure::Throw(value)) => Ok(self.fulfill(target, Err(value))?),
            Err(failure) => Err(failure),
        }
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
            match failure {
                Failure::Throw(value) => {
                    self.call(reject, vec![value], host)?;
                }
                failure => return Err(failure),
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
