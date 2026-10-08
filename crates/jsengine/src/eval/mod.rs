use super::syntax::{Expression, Statement};
use super::{Engine, Value};
use crate::Addons as Host;
use crate::{Failure, Fault};

mod assignment;
mod call;
mod class;
mod expression;
pub(crate) mod flow;
mod handler;
pub(crate) use flow::Flow;
mod iteration;
mod operation;
mod reference;

impl Engine {
    pub(crate) fn block(
        &mut self,
        body: &[Statement],
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Flow, Failure> {
        self.declarations(body, scope)?;
        let mut result = None;
        for statement in body {
            match self.statement(statement, scope, host)? {
                Flow::Next(value) => result = value.or(result),
                flow => return Ok(flow.fill(result)),
            }
        }
        Ok(Flow::Next(result))
    }

    fn statement(
        &mut self,
        statement: &Statement,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Flow, Failure> {
        if let Statement::Located(statement, site) = statement {
            let previous = self.site.replace(site.clone());
            let result = self.statement(statement, scope, host);
            self.site = previous;
            return result.map_err(|failure| failure.at(Some(site.clone())));
        }
        self.tick()?;
        match self.execution(statement, scope, host) {
            Ok(flow) => Ok(flow),
            Err(failure) if failure.thrown().is_some() => {
                Ok(Flow::Throw(failure.at(self.site.clone())))
            }
            Err(failure) => Err(failure.at(self.site.clone())),
        }
    }

    fn execution(
        &mut self,
        statement: &Statement,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Flow, Failure> {
        match statement {
            Statement::Located(..) => unreachable!("located statements enter through statement"),
            Statement::Declare { name, value, .. } => {
                let value = self.named((value, Some(name)), scope, host)?;
                self.initialize(scope, name, value);
                Ok(Flow::Next(None))
            }
            Statement::Evaluate(expression) => self
                .evaluate(expression, scope, host)
                .map(|value| Flow::Next(Some(value))),
            Statement::Block(body) => {
                let child = self.scope(scope);
                Ok(self.block(body, child, host)?)
            }
            Statement::If { condition, yes, no } => {
                let value = self.evaluate(condition, scope, host)?;
                Ok(self
                    .branch(value, (yes, no.as_deref()), scope, host)?
                    .fill(Some(Value::Undefined)))
            }
            Statement::For(iteration) => self.iteration(iteration, scope, host),
            Statement::Try(handler) => Ok(self.handler(handler, scope, host)?),
            Statement::Throw(expression) => self
                .evaluate(expression, scope, host)
                .map(|value| Flow::Throw(Failure::Throw(value).at(self.site.clone()))),
            Statement::Empty => Ok(Flow::Next(None)),
            Statement::Break => Ok(Flow::Break(None)),
            Statement::Continue => Ok(Flow::Continue(None)),
            Statement::Return(expression) => {
                let value = match expression {
                    Some(expression) => self.evaluate(expression, scope, host)?,
                    None => Value::Undefined,
                };
                Ok(Flow::Return(value))
            }
        }
    }

    fn branch(
        &mut self,
        value: Value,
        branches: (&Statement, Option<&Statement>),
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Flow, Failure> {
        if value.truth() {
            return self.statement(branches.0, scope, host);
        }
        match branches.1 {
            Some(statement) => self.statement(statement, scope, host),
            None => Ok(Flow::Next(Some(Value::Undefined))),
        }
    }
}

impl Engine {
    pub(super) fn declarations(&mut self, body: &[Statement], scope: usize) -> Result<(), Fault> {
        for statement in body {
            if let Statement::Declare { name, mutable, .. } = statement.plain() {
                self.reserve(scope, name, *mutable)?;
            }
        }
        Ok(())
    }
}
