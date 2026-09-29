use super::{Paragraph, sequence::Sequence};
use std::{ops::Range, sync::Arc};

impl Paragraph {
    pub(super) fn offset(&self, index: usize) -> usize {
        self.sequence
            .units
            .get(index)
            .map_or(self.sequence.length, |unit| unit.source.start)
    }
    pub(super) fn extract(&self, range: Range<usize>) -> Sequence {
        let source = self.offset(range.start)..self.offset(range.end);
        let mut output = self.sequence.clone();
        output.issues.clear();
        if range != (0..self.sequence.units.len()) || source != (0..self.sequence.length) {
            let mut units = self.sequence.units[range].to_vec();
            for unit in &mut units {
                unit.source.start -= source.start;
                unit.source.end -= source.start;
            }
            output.units = Arc::new(units);
            output.length = source.len();
        }
        output
    }
}
