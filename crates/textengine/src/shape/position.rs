use super::{
    Issue,
    buffer::Buffer,
    coverage::Coverage,
    plan::{Plan, Subtable},
};
use crate::{
    Scalar,
    font::{Problem, data::Data},
};
use std::collections::BTreeSet;

struct Attachment<'a> {
    table: &'a Subtable<'a>,
    bases: Coverage<'a>,
}
struct Anchors {
    base: [i32; 2],
    mark: [i32; 2],
}
impl Buffer {
    pub fn position(&mut self, plan: &Plan<'_>, scale: f64) -> Result<(), Problem> {
        let glyphs: BTreeSet<_> = self.slots.iter().map(|slot| slot.glyph.id).collect();
        let mut marks = BTreeSet::new();
        let mut lookups = Vec::new();
        for lookup in &plan.lookups {
            let mut attachments = Vec::new();
            for table in lookup {
                let bases = table.bases.clone().ok_or(Problem::Malformed)??;
                marks.extend(glyphs.iter().copied().filter(|glyph| {
                    table.coverage.index(*glyph).is_some()
                        || (table.kind == 6 && bases.index(*glyph).is_some())
                }));
                attachments.push(Attachment { table, bases });
            }
            lookups.push(attachments);
        }
        if marks.is_empty() {
            return Ok(());
        }
        let mut attached = vec![false; self.slots.len()];
        for slot in &mut self.slots {
            if marks.contains(&slot.glyph.id) {
                slot.glyph.advance = Scalar::ZERO;
            }
        }
        for lookup in lookups {
            self.attach(&lookup, (&marks, scale), &mut attached)?;
        }
        for (index, slot) in self.slots.iter().enumerate() {
            if marks.contains(&slot.glyph.id) && !attached[index] {
                self.issues.push(Issue::Unpositioned(slot.source.clone()));
            }
        }
        Ok(())
    }
    fn attach(
        &mut self,
        lookup: &[Attachment<'_>],
        context: (&BTreeSet<u16>, f64),
        attached: &mut [bool],
    ) -> Result<(), Problem> {
        for (index, attached) in attached.iter_mut().enumerate() {
            for attachment in lookup {
                self.charge(1)?;
                if attachment.apply(self, index, context)? {
                    *attached = true;
                    break;
                }
            }
        }
        Ok(())
    }
}
impl Attachment<'_> {
    fn apply(
        &self,
        buffer: &mut Buffer,
        index: usize,
        context: (&BTreeSet<u16>, f64),
    ) -> Result<bool, Problem> {
        let (marks, scale) = context;
        if self.table.filter.skip(buffer.slots[index].glyph.id)? {
            return Ok(false);
        }
        let Some(mark) = self.table.coverage.index(buffer.slots[index].glyph.id) else {
            return Ok(false);
        };
        let mut base = index;
        while base > 0 {
            buffer.charge(1)?;
            base -= 1;
            if self.table.filter.skip(buffer.slots[base].glyph.id)? {
                continue;
            }
            if self.table.kind == 6 || !marks.contains(&buffer.slots[base].glyph.id) {
                break;
            }
        }
        if base == index || self.table.filter.skip(buffer.slots[base].glyph.id)? {
            return Ok(false);
        }
        let Some(coverage) = self.bases.index(buffer.slots[base].glyph.id) else {
            return Ok(false);
        };
        let component = buffer.slots[base]
            .components
            .entries()
            .iter()
            .rposition(|range| range.start <= buffer.slots[index].glyph.source.start)
            .unwrap_or(0);
        let Some(anchors) = self.anchors(mark, coverage, component)? else {
            return Ok(false);
        };
        buffer.charge(index - base + 1)?;
        let distance = buffer.slots[base..index]
            .iter()
            .fold(Scalar::ZERO, |sum, slot| sum + slot.glyph.advance);
        let offset = buffer.slots[base].glyph.offset;
        buffer.slots[index].glyph.offset = [
            offset[0] + Scalar::new(f64::from(anchors.base[0] - anchors.mark[0]) * scale)
                - distance,
            offset[1] - Scalar::new(f64::from(anchors.base[1] - anchors.mark[1]) * scale),
        ];
        let start = buffer.slots[base].source.start;
        let end = buffer.slots[index].source.end;
        let mut first = base;
        while first > 0 && buffer.slots[first - 1].source.start == start {
            buffer.charge(1)?;
            first -= 1;
        }
        for slot in &mut buffer.slots[first..=index] {
            slot.source = start..end;
        }
        Ok(true)
    }
    fn anchors(
        &self,
        mark: usize,
        base: usize,
        component: usize,
    ) -> Result<Option<Anchors>, Problem> {
        let data = self.table.data;
        let count = usize::from(data.word(6)?);
        if count == 0 || count > 256 {
            return Err(Problem::Malformed);
        }
        let marks = data.offset(8)?;
        if mark >= usize::from(marks.word(0)?) {
            return Err(Problem::Malformed);
        }
        let class = usize::from(marks.word(2 + mark * 4)?);
        if class >= count {
            return Err(Problem::Malformed);
        }
        let bases = data.offset(10)?;
        if base >= usize::from(bases.word(0)?) {
            return Err(Problem::Malformed);
        }
        let (bases, offset) = if self.table.kind == 5 {
            let ligature = bases.offset(2 + base * 2)?;
            let components = usize::from(ligature.word(0)?);
            if components == 0 {
                return Err(Problem::Malformed);
            }
            (
                ligature,
                2 + (component.min(components - 1) * count + class) * 2,
            )
        } else {
            (bases, 2 + (base * count + class) * 2)
        };
        if bases.word(offset)? == 0 {
            return Ok(None);
        }
        Ok(Some(Anchors {
            base: anchor(bases.offset(offset)?)?,
            mark: anchor(marks.offset(4 + mark * 4)?)?,
        }))
    }
}
fn anchor(data: Data<'_>) -> Result<[i32; 2], Problem> {
    if data.word(0)? != 1 {
        return Err(Problem::Unsupported);
    }
    Ok([i32::from(data.signed(2)?), i32::from(data.signed(4)?)])
}
