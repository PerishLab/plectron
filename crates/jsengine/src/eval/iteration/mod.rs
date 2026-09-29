use super::{Engine, Flow, Host, Value};
use crate::syntax::Iteration;
use crate::{Access, Error, Failure, Invocation, value::Atom};
mod sequence;

impl Engine {
    pub(super) fn iteration(
        &mut self,
        iteration: &Iteration,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Flow, Failure> {
        let head = self.scope(scope);
        self.reserve(head, &iteration.name, iteration.mutable)?;
        let source = self.evaluate(&iteration.source, head, host)?;
        let mut cursor = Cursor::new(source, self, host)?;
        let mut result = Some(Value::Undefined);
        while let Some(value) = cursor.next(self, host)? {
            self.tick()?;
            let child = self.scope(scope);
            self.define(child, iteration.name.clone(), value, iteration.mutable)?;
            let flow = match self.statement(&iteration.body, child, host) {
                Ok(flow) => flow,
                Err(failure) => {
                    let _ = cursor.close(self, host);
                    return Err(failure.into());
                }
            };
            match flow {
                Flow::Next(value) | Flow::Continue(value) => result = value.or(result),
                Flow::Break(value) => {
                    cursor.close(self, host)?;
                    result = value.or(result);
                    break;
                }
                flow => return cursor.finish(flow, self, host),
            }
        }
        Ok(Flow::Next(result))
    }
}

pub(super) struct Cursor {
    iterator: Value,
    next: Value,
}

impl Cursor {
    pub(crate) fn trace(&self, visit: &mut dyn FnMut(&Value)) {
        visit(&self.iterator);
        visit(&self.next);
    }

    fn finish(
        &mut self,
        flow: Flow,
        engine: &mut Engine,
        host: &mut impl Host,
    ) -> Result<Flow, Failure> {
        if matches!(flow, Flow::Throw(_)) {
            let _ = self.close(engine, host);
        } else {
            self.close(engine, host)?;
        }
        Ok(flow)
    }

    pub(super) fn new(
        value: Value,
        engine: &mut Engine,
        host: &mut impl Host,
    ) -> Result<Self, Failure> {
        let method = engine.reading(
            crate::value::View {
                base: &value,
                receiver: &value,
                key: Atom::Symbol(engine.iterator),
            },
            host,
        )?;
        if method == Value::Undefined || method == Value::Null {
            return Err(engine.raise(Error::Type, "only arrays and text are iterable"));
        }
        let iterator = engine.apply(
            method,
            Invocation {
                receiver: value,
                arguments: Vec::new(),
            },
            host,
        )?;
        if !matches!(iterator, Value::Object(_)) {
            return Err(engine.raise(Error::Type, "iterator must be an object"));
        }
        let next = engine.read(&Access::direct(&iterator, "next"), host)?;
        engine.callable(&next)?;
        Ok(Self { iterator, next })
    }

    pub(super) fn next(
        &mut self,
        engine: &mut Engine,
        host: &mut impl Host,
    ) -> Result<Option<Value>, Failure> {
        let result = engine.apply(
            self.next.clone(),
            Invocation {
                receiver: self.iterator.clone(),
                arguments: Vec::new(),
            },
            host,
        )?;
        if !matches!(result, Value::Object(_)) {
            return Err(engine.raise(Error::Type, "iterator result must be an object"));
        }
        if engine.read(&Access::direct(&result, "done"), host)?.truth() {
            return Ok(None);
        }
        Ok(Some(engine.read(&Access::direct(&result, "value"), host)?))
    }

    pub(super) fn close(
        &mut self,
        engine: &mut Engine,
        host: &mut impl Host,
    ) -> Result<(), Failure> {
        let method = engine.read(&Access::direct(&self.iterator, "return"), host)?;
        if method == Value::Undefined || method == Value::Null {
            return Ok(());
        }
        let result = engine.apply(
            method,
            Invocation {
                receiver: self.iterator.clone(),
                arguments: Vec::new(),
            },
            host,
        )?;
        if !matches!(result, Value::Object(_)) {
            return Err(engine.raise(Error::Type, "iterator return must produce an object"));
        }
        Ok(())
    }
}
