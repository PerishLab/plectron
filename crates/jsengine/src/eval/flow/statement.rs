use super::{
    Flow,
    task::{Step, Task},
};
use crate::eval::iteration::Cursor;
use crate::syntax::{Handler, Statement};
use crate::{Addons, Engine, Failure, Value};

impl Task {
    fn iterating(&mut self) -> bool {
        match self.flow.take() {
            Some(Flow::Break(_)) => false,
            None | Some(Flow::Continue(_)) => true,
            flow => {
                self.flow = flow;
                false
            }
        }
    }
    pub(super) fn step(
        &mut self,
        step: Step,
        engine: &mut Engine,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        match step {
            Step::Block(body, scope) => {
                engine.declarations(&body, scope)?;
                self.steps.extend(
                    (0..body.len())
                        .rev()
                        .map(|index| Step::Source(body.clone(), index, scope)),
                );
            }
            Step::Source(body, index, scope) => {
                self.statement((body[index].clone(), scope), engine, host)?
            }
            Step::Statement(statement, scope) => {
                self.statement((statement, scope), engine, host)?
            }
            Step::Declare(name, scope) => {
                let value = self.pop();
                engine.initialize(scope, &name, value);
            }
            Step::Discard => {
                self.pop();
            }
            Step::Return => self.flow = Some(Flow::Return(self.pop())),
            Step::Throw => self.flow = Some(Flow::Throw(self.pop())),
            Step::Branch(yes, no, scope) => {
                if self.pop().truth() {
                    self.steps.push(Step::Statement(*yes, scope));
                } else if let Some(no) = no {
                    self.steps.push(Step::Statement(*no, scope));
                }
            }
            Step::Handler(handler, scope, depth) => {
                self.handler(handler, (scope, depth), engine)?
            }
            Step::Restore(previous, depth) => {
                self.truncate(depth);
                if self.flow.is_none() {
                    self.flow = previous;
                }
            }
            Step::Iterate(iteration, scope) => {
                let source = self.pop();
                let cursor = Cursor::new(source, engine, host)?;
                self.steps.push(Step::Loop(cursor, iteration, scope));
            }
            Step::Loop(mut cursor, iteration, scope) => {
                if !self.iterating() {
                    return Ok(());
                }
                if let Some(value) = cursor.next(engine, host)? {
                    let child = engine.scope(scope);
                    engine.define(child, iteration.name.clone(), value, iteration.mutable)?;
                    let body = (*iteration.body).clone();
                    self.steps.push(Step::Loop(cursor, iteration, scope));
                    self.steps.push(Step::Statement(body, child));
                }
            }
            step => self.compute(step, engine, host)?,
        }
        Ok(())
    }
    fn statement(
        &mut self,
        input: (Statement, usize),
        engine: &mut Engine,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        let (statement, scope) = input;
        match statement {
            Statement::Declare { name, value, .. } => {
                if !super::expression::suspends(&value) {
                    let value = engine.named((&value, Some(&name)), scope, host)?;
                    engine.initialize(scope, &name, value);
                    return Ok(());
                }
                self.steps.push(Step::Declare(name, scope));
                self.expression(value, scope);
            }
            Statement::Evaluate(value) => {
                self.steps.push(Step::Discard);
                self.expression(value, scope);
            }
            Statement::Block(body) => self
                .steps
                .push(Step::Block(body.into(), engine.scope(scope))),
            Statement::Return(value) => {
                self.steps.push(Step::Return);
                if let Some(value) = value {
                    self.expression(value, scope);
                } else {
                    self.values.push(Value::Undefined);
                }
            }
            Statement::Throw(value) => {
                self.steps.push(Step::Throw);
                self.expression(value, scope);
            }
            Statement::If { condition, yes, no } => {
                self.steps.push(Step::Branch(yes, no, scope));
                self.expression(condition, scope);
            }
            Statement::Try(mut handler) => {
                let body = std::mem::take(&mut handler.body);
                self.steps.push(Step::Handler(
                    handler,
                    scope,
                    (self.values.len(), self.references.len()),
                ));
                self.steps
                    .push(Step::Block(body.into(), engine.scope(scope)));
            }
            Statement::For(iteration) => {
                let head = engine.scope(scope);
                engine.reserve(head, &iteration.name, iteration.mutable)?;
                let source = iteration.source.clone();
                self.steps.push(Step::Iterate(iteration, scope));
                self.expression(source, head);
            }
            Statement::Break => self.flow = Some(Flow::Break(None)),
            Statement::Continue => self.flow = Some(Flow::Continue(None)),
            Statement::Empty => (),
        }
        Ok(())
    }
    fn handler(
        &mut self,
        mut handler: Handler,
        location: (usize, (usize, usize)),
        engine: &mut Engine,
    ) -> Result<(), Failure> {
        let (scope, depth) = location;
        self.truncate(depth);
        if matches!(self.flow, Some(Flow::Throw(_)))
            && let Some(catch) = handler.catch.take()
        {
            let Some(Flow::Throw(value)) = self.flow.take() else {
                unreachable!()
            };
            let binding = engine.scope(scope);
            if let Some(name) = catch.name {
                engine.define(binding, name, value, true)?;
            }
            self.steps.push(Step::Handler(handler, scope, depth));
            self.steps
                .push(Step::Block(catch.body.into(), engine.scope(binding)));
        } else if let Some(body) = handler.finally {
            self.steps.push(Step::Restore(self.flow.take(), depth));
            self.steps
                .push(Step::Block(body.into(), engine.scope(scope)));
        }
        Ok(())
    }
}
