use super::Cursor;
use crate::syntax::Item;
use crate::{Addons, Engine, Failure, Fault, Value};

impl Engine {
    pub(crate) fn sequence<'a>(
        &mut self,
        items: impl Iterator<Item = Option<&'a Item>>,
        scope: usize,
        host: &mut impl Addons,
    ) -> Result<Vec<Option<Value>>, Failure> {
        let mut values = Vec::new();
        for item in items {
            self.tick()?;
            match item {
                None => push(&mut values, None)?,
                Some(Item::Value(expression)) => {
                    push(&mut values, Some(self.evaluate(expression, scope, host)?))?;
                }
                Some(Item::Spread(expression)) => {
                    let value = self.evaluate(expression, scope, host)?;
                    let cursor = Cursor::new(value, self, host)?;
                    self.spread(cursor, &mut values, host)?;
                }
            }
        }
        Ok(values)
    }

    fn spread(
        &mut self,
        mut cursor: Cursor,
        values: &mut Vec<Option<Value>>,
        host: &mut impl Addons,
    ) -> Result<(), Failure> {
        while let Some(value) = cursor.next(self, host)? {
            self.tick()?;
            push(values, Some(value))?;
        }
        Ok(())
    }
}

fn push(values: &mut Vec<Option<Value>>, value: Option<Value>) -> Result<(), Fault> {
    if values.len() >= 4096 {
        return Err(Fault("collection budget exhausted".into()));
    }
    values.push(value);
    Ok(())
}
