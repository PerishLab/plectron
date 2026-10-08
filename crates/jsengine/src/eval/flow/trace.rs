use super::{Flow, Task, task::Step};
use crate::Value;
use crate::eval::reference::{Key, Reference};

impl Task {
    pub(crate) fn trace(
        &self,
        value: &mut dyn FnMut(&Value),
        scope: &mut dyn FnMut(usize),
        binding: &mut dyn FnMut(usize),
    ) {
        value(&self.target);
        scope(self.scope);
        for item in &self.values {
            value(item);
        }
        if let Some(flow) = &self.flow {
            outcome(flow, value);
        }
        for reference in &self.references {
            address(reference, value, binding);
        }
        for step in &self.steps {
            frame(step, value, scope);
        }
    }
}

fn outcome(flow: &Flow, value: &mut dyn FnMut(&Value)) {
    match flow {
        Flow::Next(Some(item))
        | Flow::Break(Some(item))
        | Flow::Continue(Some(item))
        | Flow::Return(item) => value(item),
        Flow::Throw(failure) => value(failure.thrown().expect("throw flow")),
        _ => {}
    }
}

fn address(
    reference: &Reference<'_>,
    value: &mut dyn FnMut(&Value),
    binding: &mut dyn FnMut(usize),
) {
    match reference {
        Reference::Binding {
            address: Some(address),
            ..
        } => binding(address.0),
        Reference::Property {
            base,
            key,
            receiver,
        } => {
            value(base);
            value(receiver);
            if let Key::Value(item) = key {
                value(item);
            }
        }
        _ => {}
    }
}

fn frame(step: &Step, value: &mut dyn FnMut(&Value), scope: &mut dyn FnMut(usize)) {
    match step {
        Step::Block(_, id)
        | Step::Source(_, id, _)
        | Step::Statement(_, id)
        | Step::Declare(_, id)
        | Step::Branch(_, _, id)
        | Step::Handler(_, id, _)
        | Step::Iterate(_, id)
        | Step::Expression(_, id)
        | Step::Named(_, id, _)
        | Step::Left(_, _, id)
        | Step::Reference(_, id)
        | Step::Assignment(_, _, id)
        | Step::Callee(_, id)
        | Step::Callable(_, id, _) => scope(*id),
        Step::Loop(cursor, _, id) => {
            scope(*id);
            cursor.trace(value);
        }
        Step::Super(item, id) => {
            scope(*id);
            value(item);
        }
        Step::Binary(_, item) | Step::Write(_, Some(item)) => value(item),
        Step::Restore(Some(flow), _) => outcome(flow, value),
        Step::Collect(collection) | Step::Append(collection, _) => collection.trace(value, scope),
        Step::Fields(fields) | Step::Field(fields, _) | Step::Store(fields, _) => {
            scope(fields.scope);
            value(&fields.object);
        }
        _ => {}
    }
}
