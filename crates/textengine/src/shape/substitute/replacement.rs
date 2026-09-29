use super::super::buffer::Components;
use super::{Buffer, Problem, Subtable};
use crate::Scalar;

impl Buffer {
    pub(super) fn replace(
        &mut self,
        position: usize,
        table: &Subtable<'_>,
    ) -> Result<Option<usize>, Problem> {
        let glyph = self.slots[position].glyph.id;
        let Some(index) = table.coverage.index(glyph) else {
            return Ok(None);
        };
        let data = table.data;
        match table.kind {
            1 => {
                let id = match data.word(0)? {
                    1 => glyph.wrapping_add(data.word(4)?),
                    2 if index < usize::from(data.word(4)?) => data.word(6 + index * 2)?,
                    _ => return Err(Problem::Malformed),
                };
                self.slots[position].glyph.id = id;
                Ok(Some(1))
            }
            2 => {
                if data.word(0)? != 1 || index >= usize::from(data.word(4)?) {
                    return Err(Problem::Malformed);
                }
                let sequence = data.offset(6 + index * 2)?;
                let count = usize::from(sequence.word(0)?);
                if count == 0 {
                    return Err(Problem::Unsupported);
                }
                if self.slots.len() + count > 65_536 {
                    return Err(Problem::Budget);
                }
                self.charge(count + self.slots.len() - position)?;
                let mut slots = Vec::new();
                for index in 0..count {
                    slots.push(self.duplicate(position, sequence.word(2 + index * 2)?, index == 0));
                }
                self.slots.splice(position..position + 1, slots);
                Ok(Some(count))
            }
            4 => self.ligature(position, (table, index)),
            _ => Err(Problem::Unsupported),
        }
    }
    fn duplicate(
        &mut self,
        position: usize,
        glyph: u16,
        first: bool,
    ) -> super::super::buffer::Slot {
        let mut slot = self.slots[position].clone();
        slot.glyph.id = glyph;
        if !first {
            slot.serial = self.sequence;
            self.sequence += 1;
        }
        slot
    }
    fn ligature(
        &mut self,
        position: usize,
        selected: (&Subtable<'_>, usize),
    ) -> Result<Option<usize>, Problem> {
        let (table, index) = selected;
        let data = table.data;
        if data.word(0)? != 1 || index >= usize::from(data.word(4)?) {
            return Err(Problem::Malformed);
        }
        let set = data.offset(6 + index * 2)?;
        for candidate in 0..usize::from(set.word(0)?) {
            self.charge(1)?;
            let ligature = set.offset(2 + candidate * 2)?;
            let count = usize::from(ligature.word(2)?);
            if count < 2 {
                return Err(Problem::Malformed);
            }
            let Some(indices) = self.forward(position, count, table)? else {
                continue;
            };
            let mut matched = true;
            for (component, index) in indices.iter().enumerate().skip(1) {
                if self.slots[*index].glyph.id != ligature.word(2 + component * 2)? {
                    matched = false;
                    break;
                }
            }
            if !matched {
                continue;
            }
            let end = *indices.last().unwrap();
            let range = self.slots[position].source.start..self.slots[end].source.end;
            let mut slot = self.slots[position].clone();
            slot.components = Components::Many(
                indices
                    .iter()
                    .flat_map(|index| self.slots[*index].components.entries().iter().cloned())
                    .collect(),
            );
            slot.glyph.id = ligature.word(0)?;
            slot.glyph.source = range.clone();
            slot.glyph.advance = Scalar::ZERO;
            slot.glyph.offset = [Scalar::ZERO; 2];
            self.charge((self.slots.len() - position).saturating_mul(indices.len()))?;
            for entry in &mut self.slots[position..=end] {
                entry.source = range.clone();
            }
            slot.source = range;
            for index in indices.iter().skip(1).rev() {
                self.slots.remove(*index);
            }
            self.slots[position] = slot;
            return Ok(Some(1));
        }
        Ok(None)
    }
}
