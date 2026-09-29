use super::{Engine, Expression, Host, Value};
use crate::Failure;
use crate::syntax::{Assignment, Item, Update};

impl Engine {
    pub(super) fn assignment(
        &mut self,
        assignment: &Assignment,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let reference = self.reference(&assignment.target, scope, host)?;
        let operator = assignment.operator.as_str();
        let previous = if operator == "=" {
            None
        } else {
            Some(reference.get(self, host)?)
        };
        if let Some(previous) = &previous
            && skip(operator, previous)
        {
            return Ok(previous.clone());
        }
        let logical = matches!(operator, "&&=" | "||=" | "??=");
        let name = match assignment.target.as_ref() {
            Expression::Name(name) if operator == "=" || logical => Some(name.as_str()),
            _ => None,
        };
        let value = self.named((&assignment.value, name), scope, host)?;
        let value = match previous {
            Some(previous) if !logical => {
                super::operation::binary(&operator[..operator.len() - 1], previous, value)?
            }
            _ => value,
        };
        reference.set(self, value, host)
    }

    pub(super) fn updating(
        &mut self,
        update: &Update,
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let reference = self.reference(&update.target, scope, host)?;
        let previous = reference.get(self, host)?;
        let previous = previous.number()?;
        let value = Value::Number(previous + f64::from(update.delta));
        reference.set(self, value.clone(), host)?;
        Ok(if update.prefix {
            value
        } else {
            Value::Number(previous)
        })
    }

    pub(super) fn elements(
        &mut self,
        items: &[Option<Item>],
        scope: usize,
        host: &mut impl Host,
    ) -> Result<Value, Failure> {
        let values = self.sequence(items.iter().map(Option::as_ref), scope, host)?;
        Ok(self.array(values)?)
    }
}

fn skip(operator: &str, value: &Value) -> bool {
    match operator {
        "&&=" => !value.truth(),
        "||=" => value.truth(),
        "??=" => !matches!(value, Value::Null | Value::Undefined),
        _ => false,
    }
}
