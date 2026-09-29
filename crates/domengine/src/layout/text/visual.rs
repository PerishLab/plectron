use super::paragraph::{Paragraph, Placed};
use graphics::shared::Scalar;
use std::ops::Range;

impl Paragraph {
    pub(super) fn visual(&self, range: Range<usize>) -> (Vec<Placed>, u8) {
        let clusters = &self.sequence.units[range.clone()];
        let Some(first) = clusters.first() else {
            return (Vec::new(), self.bidi.base(self.origin));
        };
        let units = self.bidi.line(
            self.origin + first.source.start..self.origin + clusters.last().unwrap().source.end,
        );
        let levels: Vec<_> = clusters
            .iter()
            .map(|cluster| {
                let index = units.partition_point(|(index, _)| {
                    self.bidi.units[*index].source.end <= self.origin + cluster.source.start
                });
                units[index].1
            })
            .collect();
        let mut x = Scalar::ZERO;
        let visual = textengine::bidi::reorder(&levels)
            .into_iter()
            .map(|index| {
                let placed = Placed {
                    unit: range.start + index,
                    x,
                    level: levels[index],
                };
                x = x + clusters[index].advance;
                placed
            })
            .collect();
        (visual, self.bidi.base(self.origin + first.source.start))
    }
}
