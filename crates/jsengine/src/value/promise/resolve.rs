use super::{Job, Outcome, Reaction};
use crate::{Access, Addons, Engine, Error, Failure, Fault, Value};

impl Engine {
    pub(crate) fn settle(
        &mut self,
        id: usize,
        result: (bool, Value),
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let (promise, used) = self.promises.resolvers[id].as_mut().expect("live resolver");
        if *used {
            return Ok(());
        }
        *used = true;
        let promise = promise.clone();
        if result.0 {
            self.fulfill(&promise, Err(result.1))?;
        } else {
            self.adopt(&promise, result.1, host)?;
        }
        Ok(())
    }
    pub(crate) fn adopt(
        &mut self,
        promise: &Value,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        if *promise == value {
            let error =
                self.exception(Error::Type, Some("promise cannot resolve to itself".into()))?;
            return Ok(self.fulfill(promise, Err(error))?);
        }
        if !matches!(value, Value::Object(_)) {
            return Ok(self.fulfill(promise, Ok(value))?);
        }
        let then = match self.read(&Access::direct(&value, "then"), host) {
            Ok(then) => then,
            Err(Failure::Throw(value)) => return Ok(self.fulfill(promise, Err(value))?),
            Err(failure) => return Err(failure),
        };
        if self.callable(&then).is_err() {
            return Ok(self.fulfill(promise, Ok(value))?);
        }
        Ok(self.enqueue(Job::Assimilate {
            target: promise.clone(),
            source: value,
            then,
        })?)
    }
    pub(crate) fn fulfill(&mut self, promise: &Value, outcome: Outcome) -> Result<(), Fault> {
        let slot = self.slot(promise)?;
        let entry = self.promises.entries.get_mut(&slot).expect("promise slot");
        if entry.outcome.is_some() {
            return Ok(());
        }
        entry.outcome = Some(outcome.clone());
        let reactions = std::mem::take(&mut entry.reactions);
        for reaction in reactions {
            self.enqueue(Job::React(reaction, outcome.clone()))?;
        }
        Ok(())
    }
    pub(crate) fn suspend(
        &mut self,
        task: crate::eval::flow::Task,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let promise = self.resolved(value, host)?;
        Ok(self.subscribe(&promise, Reaction::Resume(Box::new(task)))?)
    }
}

impl Engine {
    pub(crate) fn resolved(
        &mut self,
        value: Value,
        host: &mut impl Addons,
    ) -> Result<Value, Failure> {
        self.validate(&value)?;
        if self.slot(&value).is_ok() {
            return Ok(value);
        }
        let promise = self.promise()?;
        self.adopt(&promise, value, host)?;
        Ok(promise)
    }
}
