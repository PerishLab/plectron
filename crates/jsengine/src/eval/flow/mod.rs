use crate::Value;
mod collect;
mod expression;
mod statement;
mod task;
mod trace;
pub(crate) use task::Task;

pub(crate) enum Flow {
    Next(Option<Value>),
    Return(Value),
    Break(Option<Value>),
    Continue(Option<Value>),
    Throw(Value),
}

impl Flow {
    pub fn fill(self, previous: Option<Value>) -> Self {
        match self {
            Self::Next(value) => Self::Next(value.or(previous)),
            Self::Break(value) => Self::Break(value.or(previous)),
            Self::Continue(value) => Self::Continue(value.or(previous)),
            flow => flow,
        }
    }
}
