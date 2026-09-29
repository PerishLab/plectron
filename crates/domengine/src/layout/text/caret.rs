use crate::layout::measure::View;
use graphics::shared::Scalar;
use std::ops::Range;

impl View<'_> {
    pub(in crate::layout) fn position(&self, offset: usize) -> usize {
        let lines = self.text.as_ref().expect("editor text is shaped");
        let Some(line) = lines.ranges.first() else {
            return 0;
        };
        let offset = offset.saturating_sub(line.source.start);
        let index = line
            .sequence
            .units
            .partition_point(|cluster| cluster.source.end < offset);
        let Some(placed) = line.visual.iter().find(|placed| placed.unit == index) else {
            return line.width.ceil();
        };
        let cluster = &line.sequence.units[index];
        let distance = progress(self, index, offset).min(cluster.advance);
        let x = if placed.level.is_multiple_of(2) {
            placed.x + distance
        } else {
            placed.x + cluster.advance - distance
        };
        x.min(line.width).ceil()
    }
    pub(in crate::layout) fn intervals(&self, selection: Range<usize>) -> Vec<Range<usize>> {
        let lines = self.text.as_ref().expect("editor text is shaped");
        let Some(line) = lines.ranges.first() else {
            return Vec::new();
        };
        let selection = selection.start.saturating_sub(line.source.start)
            ..selection.end.saturating_sub(line.source.start);
        let mut intervals: Vec<Range<usize>> = Vec::new();
        for placed in &line.visual {
            let cluster = &line.sequence.units[placed.unit];
            let range =
                selection.start.max(cluster.source.start)..selection.end.min(cluster.source.end);
            if range.is_empty() {
                continue;
            }
            let start = progress(self, placed.unit, range.start);
            let end = progress(self, placed.unit, range.end);
            let interval = if placed.level.is_multiple_of(2) {
                placed.x + start..placed.x + end
            } else {
                placed.x + cluster.advance - end..placed.x + cluster.advance - start
            };
            let interval =
                interval.start.min(line.width).ceil()..interval.end.min(line.width).ceil();
            if interval.is_empty() {
                continue;
            }
            if let Some(previous) = intervals
                .last_mut()
                .filter(|previous| previous.end >= interval.start)
            {
                previous.end = previous.end.max(interval.end);
            } else {
                intervals.push(interval);
            }
        }
        intervals
    }
}
fn progress(measured: &View<'_>, index: usize, offset: usize) -> Scalar {
    let line = &measured.text.as_ref().unwrap().ranges[0];
    let unit = &line.sequence.units[index];
    let (part, cluster) = line.sequence.text(index).expect("editor text item");
    let run = &part.run;
    if offset >= unit.source.end {
        return cluster.advance;
    }
    if offset <= unit.source.start {
        return Scalar::ZERO;
    }
    let origin = line.source.start + unit.source.start - cluster.source.start;
    let source = &measured.material().text().unwrap()[origin..];
    let offset = offset + cluster.source.start - unit.source.start;
    run.glyphs[cluster.glyphs.clone()]
        .iter()
        .map(|glyph| position(glyph, source, offset))
        .fold(Scalar::ZERO, |sum, value| sum + value)
}
fn position(glyph: &textengine::Glyph, source: &str, offset: usize) -> Scalar {
    if offset >= glyph.source.end {
        return glyph.advance;
    }
    if offset <= glyph.source.start {
        return Scalar::ZERO;
    }
    let total = source[glyph.source.clone()].chars().count();
    let part = source[glyph.source.start..offset].chars().count();
    glyph.advance * part / total
}
