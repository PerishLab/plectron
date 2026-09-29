use super::{Bidi, Class};
use std::ops::Range;

impl Bidi {
    pub fn line(&self, range: Range<usize>) -> Vec<(usize, u8)> {
        let start = self
            .units
            .partition_point(|unit| unit.source.end <= range.start);
        let end = self
            .units
            .partition_point(|unit| unit.source.start < range.end);
        let units = &self.units[start..end];
        let mut levels: Vec<_> = units.iter().map(|unit| unit.level).collect();
        let mut trailing = true;
        for index in (0..units.len()).rev() {
            let class = units[index].class;
            let base = self.base(units[index].source.start);
            if matches!(class, Class::B | Class::S) {
                trailing = true;
                levels[index] = base;
            } else if reset(class) {
                if trailing {
                    levels[index] = base;
                }
            } else {
                trailing = false;
            }
        }
        levels
            .into_iter()
            .enumerate()
            .map(|(index, level)| (start + index, level))
            .collect()
    }
    pub fn base(&self, offset: usize) -> u8 {
        let index = self
            .paragraphs
            .partition_point(|paragraph| paragraph.source.end <= offset);
        self.paragraphs
            .get(index)
            .or(self.paragraphs.last())
            .map_or(0, |paragraph| paragraph.level)
    }
    pub fn visual(&self, range: Range<usize>) -> Vec<usize> {
        self.paragraphs
            .iter()
            .flat_map(|paragraph| {
                let start = range.start.max(paragraph.source.start);
                let end = range.end.min(paragraph.source.end);
                self.permutation(start..end.max(start))
            })
            .collect()
    }
    fn permutation(&self, range: Range<usize>) -> Vec<usize> {
        let entries = self.line(range);
        let visible: Vec<_> = entries
            .into_iter()
            .filter(|(index, _)| self.units[*index].class != Class::Bn)
            .collect();
        reorder(&visible.iter().map(|(_, level)| *level).collect::<Vec<_>>())
            .into_iter()
            .map(|index| visible[index].0)
            .collect()
    }
}
fn reset(class: Class) -> bool {
    matches!(
        class,
        Class::Ws | Class::Bn | Class::Pdi | Class::Lri | Class::Rli | Class::Fsi
    )
}
pub fn reorder(levels: &[u8]) -> Vec<usize> {
    let mut indices: Vec<_> = (0..levels.len()).collect();
    let Some(minimum) = levels.iter().copied().filter(|level| level % 2 == 1).min() else {
        return indices;
    };
    let maximum = *levels.iter().max().unwrap();
    for level in (minimum..=maximum).rev() {
        let mut start = 0;
        while start < indices.len() {
            if levels[indices[start]] < level {
                start += 1;
                continue;
            }
            let mut end = start + 1;
            while end < indices.len() && levels[indices[end]] >= level {
                end += 1;
            }
            indices[start..end].reverse();
            start = end;
        }
    }
    indices
}
