use super::Flow;
use crate::eval::{iteration::Cursor, reference::Reference};
use crate::syntax::{Expression, Handler, Item, Iteration, Statement};
use crate::{Addons, Engine, Failure, Fault, Value};
use std::sync::Arc;

pub(crate) struct Task {
    pub(super) site: Option<crate::Site>,
    pub target: Value,
    pub(super) scope: usize,
    pub(super) steps: Vec<Step>,
    pub values: Vec<Value>,
    pub(super) references: Vec<Reference<'static>>,
    pub flow: Option<Flow>,
    pub waiting: bool,
}
pub(super) enum Step {
    Site(Option<crate::Site>),
    Block(Arc<[Statement]>, usize),
    Source(Arc<[Statement]>, usize, usize),
    Statement(Statement, usize),
    Declare(String, usize),
    Discard,
    Return,
    Throw,
    Branch(Box<Statement>, Option<Box<Statement>>, usize),
    Handler(Handler, usize, (usize, usize)),
    Restore(Option<Flow>, (usize, usize)),
    Iterate(Iteration, usize),
    Loop(Cursor, Iteration, usize),
    Expression(Expression, usize),
    Named(Expression, usize, Option<String>),
    Await,
    Unary(String),
    Left(String, Expression, usize),
    Binary(String, Value),
    Reference(Expression, usize),
    Property,
    Super(Value, usize),
    Get,
    Assignment(String, Expression, usize),
    Write(String, Option<Value>),
    Update(i8, bool),
    Callee(Vec<Item>, usize),
    Callable(Vec<Item>, usize, bool),
    Collect(super::collect::Collection),
    Append(super::collect::Collection, bool),
    Fields(super::collect::Fields),
    Field(super::collect::Fields, crate::syntax::Body),
    Store(super::collect::Fields, crate::value::Atom),
}
impl Task {
    pub fn new(target: Value, body: Arc<[Statement]>, scope: usize) -> Self {
        Self {
            site: None,
            target,
            scope,
            steps: vec![Step::Block(body, scope)],
            values: Vec::new(),
            references: Vec::new(),
            flow: None,
            waiting: false,
        }
    }
    pub fn pop(&mut self) -> Value {
        self.values.pop().expect("continuation operand")
    }
    pub fn expression(&mut self, expression: Expression, scope: usize) {
        self.steps.push(Step::Expression(expression, scope));
    }
    pub fn truncate(&mut self, depth: (usize, usize)) {
        self.values.truncate(depth.0);
        self.references.truncate(depth.1);
    }
}
impl Engine {
    pub(crate) fn resume(
        &mut self,
        mut task: Task,
        outcome: Result<Value, Failure>,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        if task.waiting {
            task.waiting = false;
            match outcome {
                Ok(value) => task.values.push(value),
                Err(value) => task.flow = Some(Flow::Throw(value)),
            }
        }
        while let Some(step) = task.steps.pop() {
            self.tick()
                .map_err(|fault| Failure::from(fault).at(task.site.clone()))?;
            if task.flow.is_some()
                && !matches!(
                    step,
                    Step::Handler(..) | Step::Restore(..) | Step::Loop(..) | Step::Site(..)
                )
            {
                continue;
            }
            if matches!(step, Step::Await) {
                let value = task.pop();
                task.waiting = true;
                let previous = std::mem::replace(&mut self.site, task.site.clone());
                let site = task.site.clone();
                let result = self.suspend(task, value, host);
                self.site = previous;
                return result.map_err(|failure| failure.at(site));
            }
            let previous = std::mem::replace(&mut self.site, task.site.clone());
            let result = task.step(step, self, host);
            self.site = previous;
            if let Err(failure) = result {
                let failure = failure.at(task.site.clone());
                if failure.thrown().is_some() {
                    task.flow = Some(Flow::Throw(failure));
                } else {
                    return Err(failure);
                }
            }
        }
        let result = match task.flow {
            None | Some(Flow::Next(_)) => Ok(Value::Undefined),
            Some(Flow::Return(value)) => Ok(value),
            Some(Flow::Throw(failure)) => Err(failure),
            _ => Err(Fault("loop control escaped async function".into()).into()),
        };
        self.finish(&task.target, result, host)
    }
}
