use super::{Buffer, Plan, Problem, Subtable};
use crate::font::data::Data;
use crate::shape::coverage::Coverage;

impl Buffer {
    pub(super) fn context(
        &mut self,
        position: usize,
        table: &Subtable<'_>,
        context: (&Plan<'_>, usize),
    ) -> Result<Option<usize>, Problem> {
        let data = table.data;
        match data.word(0)? {
            3 => self.covered(position, table, context),
            1 if table.kind == 6 => self.rules(position, table, context),
            _ => Err(Problem::Unsupported),
        }
    }
    fn covered(
        &mut self,
        position: usize,
        table: &Subtable<'_>,
        context: (&Plan<'_>, usize),
    ) -> Result<Option<usize>, Problem> {
        let data = table.data;
        let mut offset = 2;
        if table.kind == 6 {
            let count = usize::from(data.word(offset)?);
            offset += 2;
            let Some(indices) = self.backward(position, count, table)? else {
                return Ok(None);
            };
            if !self.coverages(data, &indices, offset)? {
                return Ok(None);
            }
            offset += count * 2;
        }
        let count = usize::from(data.word(offset)?);
        offset += 2;
        if count == 0 {
            return Err(Problem::Malformed);
        }
        let records = if table.kind == 5 {
            let value = data.word(offset)?;
            offset += 2;
            Some(value)
        } else {
            None
        };
        let Some(indices) = self.forward(position, count, table)? else {
            return Ok(None);
        };
        if !self.coverages(data, &indices, offset)? {
            return Ok(None);
        }
        offset += count * 2;
        if table.kind == 6 {
            let count = usize::from(data.word(offset)?);
            offset += 2;
            let Some(ahead) = self.forward(indices.last().unwrap() + 1, count, table)? else {
                return Ok(None);
            };
            if !self.coverages(data, &ahead, offset)? {
                return Ok(None);
            }
            offset += count * 2;
        }
        let count = match records {
            Some(count) => count,
            None => data.word(offset)?,
        };
        if records.is_none() {
            offset += 2;
        }
        self.records(data.tail(offset)?, (count, indices), context)?;
        Ok(Some(1))
    }
    fn rules(
        &mut self,
        position: usize,
        table: &Subtable<'_>,
        context: (&Plan<'_>, usize),
    ) -> Result<Option<usize>, Problem> {
        let Some(index) = table.coverage.index(self.slots[position].glyph.id) else {
            return Ok(None);
        };
        if index >= usize::from(table.data.word(4)?) {
            return Err(Problem::Malformed);
        }
        let offset = 6 + index * 2;
        if table.data.word(offset)? == 0 {
            return Ok(None);
        }
        let set = table.data.offset(offset)?;
        for rule in 0..usize::from(set.word(0)?) {
            self.charge(1)?;
            let data = set.offset(2 + rule * 2)?;
            let count = usize::from(data.word(0)?);
            let Some(back) = self.backward(position, count, table)? else {
                continue;
            };
            if !self.literal(data, &back, 2)? {
                continue;
            }
            let mut offset = 2 + count * 2;
            let count = usize::from(data.word(offset)?);
            offset += 2;
            if count == 0 {
                return Err(Problem::Malformed);
            }
            let Some(indices) = self.forward(position, count, table)? else {
                continue;
            };
            if !self.literal(data, &indices[1..], offset)? {
                continue;
            }
            offset += (count - 1) * 2;
            let count = usize::from(data.word(offset)?);
            offset += 2;
            let Some(ahead) = self.forward(indices.last().unwrap() + 1, count, table)? else {
                continue;
            };
            if !self.literal(data, &ahead, offset)? {
                continue;
            }
            offset += count * 2;
            self.records(
                data.tail(offset + 2)?,
                (data.word(offset)?, indices),
                context,
            )?;
            return Ok(Some(1));
        }
        Ok(None)
    }
    fn coverages(
        &mut self,
        data: Data<'_>,
        indices: &[usize],
        offset: usize,
    ) -> Result<bool, Problem> {
        for (index, slot) in indices.iter().enumerate() {
            self.charge(1)?;
            let coverage = Coverage::new(data.offset(offset + index * 2)?)?;
            self.charge(coverage.len())?;
            if coverage.index(self.slots[*slot].glyph.id).is_none() {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn literal(
        &mut self,
        data: Data<'_>,
        indices: &[usize],
        offset: usize,
    ) -> Result<bool, Problem> {
        for (index, slot) in indices.iter().enumerate() {
            self.charge(1)?;
            if self.slots[*slot].glyph.id != data.word(offset + index * 2)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
    fn records(
        &mut self,
        data: Data<'_>,
        matched: (u16, Vec<usize>),
        context: (&Plan<'_>, usize),
    ) -> Result<(), Problem> {
        let (count, indices) = matched;
        let identities: Vec<_> = indices
            .iter()
            .map(|index| self.slots[*index].serial)
            .collect();
        for record in 0..usize::from(count) {
            let index = usize::from(data.word(record * 4)?);
            let id = *identities.get(index).ok_or(Problem::Malformed)?;
            self.charge(self.slots.len())?;
            if let Some(position) = self.slots.iter().position(|slot| slot.serial == id) {
                let lookup = context.0.nested(data.word(record * 4 + 2)?)?;
                self.apply(position, &lookup, (context.0, context.1 + 1))?;
            }
        }
        Ok(())
    }
}
