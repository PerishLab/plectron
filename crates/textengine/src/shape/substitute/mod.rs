mod context;
mod replacement;
use super::{
    buffer::Buffer,
    plan::{Plan, Subtable},
};
use crate::font::Problem;

impl Buffer {
    pub fn substitute(&mut self, plan: &Plan<'_>) -> Result<(), Problem> {
        for lookup in &plan.lookups {
            let mut index = 0;
            while index < self.slots.len() {
                self.charge(1)?;
                if plan.feature.is_some_and(|feature| {
                    matches!(&feature, b"isol" | b"init" | b"medi" | b"fina")
                        && self.slots[index].form != Some(feature)
                }) {
                    index += 1;
                    continue;
                }
                index += self.apply(index, lookup, (plan, 0))?.unwrap_or(1);
            }
        }
        Ok(())
    }
    fn apply(
        &mut self,
        index: usize,
        lookup: &[Subtable<'_>],
        context: (&Plan<'_>, usize),
    ) -> Result<Option<usize>, Problem> {
        if context.1 > 16 {
            return Err(Problem::Budget);
        }
        for table in lookup {
            self.charge(1)?;
            if table.filter.skip(self.slots[index].glyph.id)? {
                continue;
            }
            let result = if matches!(table.kind, 5 | 6) {
                self.context(index, table, context)?
            } else {
                self.replace(index, table)?
            };
            if result.is_some() {
                return Ok(result);
            }
        }
        Ok(None)
    }
    fn forward(
        &mut self,
        start: usize,
        count: usize,
        table: &Subtable<'_>,
    ) -> Result<Option<Vec<usize>>, Problem> {
        let mut indices = Vec::new();
        for index in start..self.slots.len() {
            if indices.len() == count {
                break;
            }
            self.charge(1)?;
            if !table.filter.skip(self.slots[index].glyph.id)? {
                indices.push(index);
            }
        }
        Ok((indices.len() == count).then_some(indices))
    }
    fn backward(
        &mut self,
        start: usize,
        count: usize,
        table: &Subtable<'_>,
    ) -> Result<Option<Vec<usize>>, Problem> {
        let mut indices = Vec::new();
        for index in (0..start).rev() {
            if indices.len() == count {
                break;
            }
            self.charge(1)?;
            if !table.filter.skip(self.slots[index].glyph.id)? {
                indices.push(index);
            }
        }
        Ok((indices.len() == count).then_some(indices))
    }
}
