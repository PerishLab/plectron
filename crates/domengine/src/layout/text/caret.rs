use crate::layout::measure::View;
use graphics::shared::Scalar;
use std::ops::Range;

impl View<'_> {
    pub(in crate::layout) fn caret(&self, offset: usize) -> (usize, usize) {
        let lines = self.text.as_ref().expect("editor text is shaped");
        if lines.ranges.is_empty() {
            return (0, 0);
        }
        let line = lines
            .ranges
            .partition_point(|range| range.source.start <= offset)
            .saturating_sub(1);
        (line, self.advance(line, offset))
    }

    pub(in crate::layout) fn intervals(
        &self,
        row: usize,
        selection: Range<usize>,
    ) -> Vec<Range<usize>> {
        let lines = self.text.as_ref().expect("editor text is shaped");
        let Some(line) = lines.ranges.get(row) else {
            return Vec::new();
        };
        if line.source.is_empty() {
            if selection.start <= line.source.start && selection.end > line.source.start {
                return std::iter::once(0..4).collect();
            }
            return Vec::new();
        }
        let start = selection.start.max(line.source.start) - line.source.start;
        let end = selection
            .end
            .min(line.source.end)
            .saturating_sub(line.source.start);
        if start >= end {
            return Vec::new();
        }
        let local = start..end;
        let mut intervals: Vec<Range<usize>> = Vec::new();
        for placed in &line.visual {
            let cluster = &line.sequence.units[placed.unit];
            let range = local.start.max(cluster.source.start)..local.end.min(cluster.source.end);
            if range.is_empty() {
                continue;
            }
            let leading = progress(self, row, placed.unit, range.start);
            let trailing = progress(self, row, placed.unit, range.end);
            let interval = if placed.level.is_multiple_of(2) {
                placed.x + leading..placed.x + trailing
            } else {
                placed.x + cluster.advance - trailing..placed.x + cluster.advance - leading
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

    pub(in crate::layout) fn offset(&self, row: usize, x: usize) -> usize {
        let lines = self.text.as_ref().expect("editor text is shaped");
        let Some(line) = lines.ranges.get(row) else {
            return 0;
        };
        if line.visual.is_empty() {
            return line.source.start;
        }
        let target = Scalar::from(x as i64);
        for placed in &line.visual {
            let cluster = &line.sequence.units[placed.unit];
            let ltr = placed.level.is_multiple_of(2);
            if target <= placed.x {
                return line.source.start
                    + if ltr {
                        cluster.source.start
                    } else {
                        cluster.source.end
                    };
            }
            if target < placed.x + cluster.advance {
                let distance = if ltr {
                    target - placed.x
                } else {
                    placed.x + cluster.advance - target
                };
                return line.source.start + nearest(self, row, placed.unit, distance);
            }
        }
        let last = line.visual.last().unwrap();
        let cluster = &line.sequence.units[last.unit];
        line.source.start
            + if last.level.is_multiple_of(2) {
                cluster.source.end
            } else {
                cluster.source.start
            }
    }

    pub(in crate::layout) fn hit(&self, point: [usize; 2]) -> usize {
        let lines = self.text.as_ref().expect("editor text is shaped");
        if lines.ranges.is_empty() {
            return 0;
        }
        let row = lines
            .ranges
            .partition_point(|line| line.y + line.height <= point[1])
            .min(lines.ranges.len() - 1);
        let line = &lines.ranges[row];
        let remaining = self.size()[0].saturating_sub(line.width.ceil());
        let alignment = self
            .material()
            .style()
            .typography
            .align
            .offset(remaining, line.direction % 2 == 1)
            .saturating_sub(usize::from(
                self.material().editor().is_some() && line.direction % 2 == 1,
            ));
        self.offset(row, point[0].saturating_sub(alignment))
    }

    pub(in crate::layout) fn vertical(
        &self,
        offset: usize,
        down: bool,
        column: Option<usize>,
    ) -> (usize, usize) {
        let lines = self.text.as_ref().expect("editor text is shaped");
        if lines.ranges.is_empty() {
            return (0, 0);
        }
        let (row, x) = self.caret(offset);
        let target = column.unwrap_or(x);
        let next = if down {
            (row + 1 < lines.ranges.len()).then_some(row + 1)
        } else {
            row.checked_sub(1)
        };
        let Some(next) = next else {
            return (offset, target);
        };
        (self.offset(next, target), target)
    }

    fn advance(&self, row: usize, offset: usize) -> usize {
        let line = &self.text.as_ref().unwrap().ranges[row];
        let offset = offset.saturating_sub(line.source.start);
        let index = line
            .sequence
            .units
            .partition_point(|cluster| cluster.source.end < offset);
        let Some(placed) = line.visual.iter().find(|placed| placed.unit == index) else {
            return line.width.ceil();
        };
        let cluster = &line.sequence.units[index];
        let distance = progress(self, row, index, offset).min(cluster.advance);
        let x = if placed.level.is_multiple_of(2) {
            placed.x + distance
        } else {
            placed.x + cluster.advance - distance
        };
        x.min(line.width).ceil()
    }
}

fn nearest(measured: &View<'_>, row: usize, index: usize, distance: Scalar) -> usize {
    let line = &measured.text.as_ref().unwrap().ranges[row];
    let unit = &line.sequence.units[index];
    let text = measured.material().text().unwrap();
    let slice = &text[line.source.start + unit.source.start..line.source.start + unit.source.end];
    let mut best = unit.source.start;
    let mut delta = (distance - Scalar::ZERO).value().abs();
    for (offset, _) in slice.char_indices().skip(1) {
        let candidate = unit.source.start + offset;
        let current = (distance - progress(measured, row, index, candidate))
            .value()
            .abs();
        if current < delta {
            delta = current;
            best = candidate;
        }
    }
    let trailing = (distance - unit.advance).value().abs();
    if trailing < delta {
        unit.source.end
    } else {
        best
    }
}

fn progress(measured: &View<'_>, row: usize, index: usize, offset: usize) -> Scalar {
    let line = &measured.text.as_ref().unwrap().ranges[row];
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
