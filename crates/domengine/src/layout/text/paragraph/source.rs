use super::sequence::{Item, Sequence, Unit};
use crate::Fault;
use std::{ops::Range, sync::Arc};
use textengine::{Prepared, bidi::Bidi};

pub(in crate::layout) struct Source {
    source: Arc<str>,
    parts: Vec<(Range<usize>, Prepared)>,
    atoms: Vec<(Range<usize>, [usize; 2])>,
}
impl Source {
    pub fn cost(&self) -> shared::cache::Cost {
        let mut cost = shared::cache::Cost::new(
            std::mem::size_of::<Self>()
                + self.source.len()
                + std::mem::size_of_val(self.parts.as_slice())
                + std::mem::size_of_val(self.atoms.as_slice()),
        );
        for (_, part) in &self.parts {
            cost.extend(&part.cost());
        }
        cost
    }
    pub fn new(
        source: Arc<str>,
        parts: Vec<(Range<usize>, Prepared)>,
        atoms: Vec<(Range<usize>, [usize; 2])>,
    ) -> Self {
        Self {
            source,
            parts,
            atoms,
        }
    }
    pub fn ranges(&self) -> Vec<Range<usize>> {
        let mut ranges = Vec::new();
        for (source, _) in &self.parts {
            ranges.extend(
                textengine::unicode::graphemes(&self.source[source.clone()])
                    .into_iter()
                    .map(|range| source.start + range.start..source.start + range.end),
            );
        }
        ranges.extend(self.atoms.iter().map(|(range, _)| range.clone()));
        ranges.sort_by_key(|range| range.start);
        ranges
    }
    pub fn shape(
        &self,
        range: Range<usize>,
        bidi: &Bidi,
        trace: &crate::layout::cache::Trace,
    ) -> Result<Sequence, Fault> {
        let mut result = Sequence::new(range.len());
        let mut remaining = [65_536usize; 3];
        let index = self
            .parts
            .partition_point(|(source, _)| source.end <= range.start);
        for (source, prepared) in self.parts[index..]
            .iter()
            .take_while(|(source, _)| source.start < range.end)
        {
            let start = source.start.max(range.start);
            let end = source.end.min(range.end);
            if start >= end {
                continue;
            }
            trace.shape(end - start);
            let mut run = prepared.shape(start - source.start..end - source.start)?;
            run.project(&self.source[start..end], bidi, start)?;
            for (budget, count) in
                remaining
                    .iter_mut()
                    .zip([run.glyphs.len(), run.spans.len(), run.issues.len()])
            {
                *budget = budget
                    .checked_sub(count)
                    .ok_or_else(|| Fault("paragraph shaping budget exhausted".into()))?;
            }
            result.append(start - range.start..end - range.start, run);
        }
        let index = self
            .atoms
            .partition_point(|(source, _)| source.end <= range.start);
        for (index, (source, size)) in self
            .atoms
            .iter()
            .enumerate()
            .skip(index)
            .take_while(|(_, (source, _))| source.start < range.end)
        {
            Arc::make_mut(&mut result.units).push(Unit {
                source: source.start - range.start..source.end - range.start,
                advance: (size[0] as i64).into(),
                item: Item::Atom { index, size: *size },
            });
        }
        if result.parts.is_empty() {
            let (_, prepared) = &self.parts[0];
            trace.shape(0);
            result.append(0..0, prepared.shape(0..0)?);
        }
        Arc::make_mut(&mut result.units).sort_by_key(|unit| unit.source.start);
        Ok(result)
    }
}
