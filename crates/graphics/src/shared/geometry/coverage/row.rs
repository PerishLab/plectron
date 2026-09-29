use super::Coverage;
use crate::shared::Fault;
use std::ops::Range;

#[derive(Default)]
pub(super) struct Row {
    deltas: Vec<(usize, i32)>,
}
impl Row {
    pub fn interval(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let first = range.start / 4;
        let last = range.end / 4;
        if first == last {
            self.add(first..first + 1, range.len() as i32);
            return;
        }
        self.add(first..first + 1, (4 - range.start % 4) as i32);
        self.add(first + 1..last, 4);
        self.add(last..last + 1, (range.end % 4) as i32);
    }
    fn add(&mut self, range: Range<usize>, value: i32) {
        if range.is_empty() || value == 0 {
            return;
        }
        self.deltas
            .extend([(range.start, value), (range.end, -value)]);
    }
    pub fn finish(&mut self, y: usize, coverage: &mut Coverage) -> Result<(), Fault> {
        self.deltas.sort_unstable_by_key(|event| event.0);
        let mut count = 0;
        let mut previous = 0;
        for &(x, delta) in &self.deltas {
            if x > previous {
                if !(0..=16).contains(&count) {
                    return Err(Fault("invalid scanline coverage".into()));
                }
                coverage.emit(y, previous..x, count as u8)?;
            }
            count += delta;
            previous = x;
        }
        if count != 0 {
            return Err(Fault("unbalanced coverage intervals".into()));
        }
        self.deltas.clear();
        Ok(())
    }
}
