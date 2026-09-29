use super::paragraph::{Line, Paragraph, kinds};
use crate::{Fault, style::typography::Wrapping};
use graphics::shared::Scalar;
use std::ops::Range;

struct Fitter<'a> {
    paragraph: &'a Paragraph,

    ranges: Vec<Range<usize>>,
    opportunities: Vec<bool>,
    hard: Vec<bool>,
    limit: Scalar,
    remaining: usize,
}
impl Paragraph {
    pub(super) fn refines(&self) -> bool {
        if !self.style.whitespace.wrap() || self.style.wrapping != Wrapping::Anywhere {
            return false;
        }
        let graphemes = self.prepared.ranges();
        self.sequence.units.iter().any(|cluster| {
            let index = graphemes.partition_point(|range| range.start < cluster.source.start);
            graphemes[index].end < cluster.source.end
        })
    }
    pub(super) fn reflow(&self, limit: Scalar) -> Result<Vec<Line>, Fault> {
        let ranges = self.prepared.ranges();
        let analyzed = super::breaking::normalize(&self.source, self.style.whitespace);
        let opportunities = super::breaking::opportunities(&ranges, &analyzed, &self.style)?;
        let hard = ranges
            .iter()
            .map(|range| {
                analyzed[range.clone()]
                    .chars()
                    .all(textengine::breaking::mandatory)
            })
            .collect();
        let mut fitter = Fitter {
            paragraph: self,
            ranges,
            opportunities,
            hard,
            limit,
            remaining: 4_194_304,
        };
        let mut lines = Vec::new();
        let mut start = 0;
        while start < fitter.ranges.len() {
            if self.style.whitespace.collapse()
                && self.source[fitter.ranges[start].clone()]
                    .chars()
                    .all(|character| matches!(character, ' ' | '\t' | '\r' | '\n' | '\u{c}'))
            {
                start += 1;
                continue;
            }
            let (line, next) = fitter.line(start)?;
            lines.push(line.finish());
            start = next;
        }
        if fitter.hard.last() == Some(&true) {
            lines.push(fitter.measure(start..start)?.finish());
        }
        Ok(lines)
    }
}
impl Fitter<'_> {
    fn line(&mut self, start: usize) -> Result<(Candidate, usize), Fault> {
        let end = (start..self.ranges.len())
            .find(|index| self.hard[*index])
            .unwrap_or(self.ranges.len());
        let next = end + usize::from(end < self.ranges.len());
        if start == end {
            return Ok((self.measure(start..end)?, next));
        }
        if self.limit == Scalar::ZERO {
            let (line, boundary) = self.atom(start..end)?;
            return Ok((line, if boundary == end { next } else { boundary }));
        }
        let mut best = None;
        for boundary in start + 1..=end {
            if boundary != end && !self.opportunities[boundary] {
                continue;
            }
            let line = self.measure(start..boundary)?;
            if line.width <= self.limit {
                if line.width > Scalar::ZERO || boundary == end {
                    best = Some((line, boundary));
                }
                continue;
            }
            let (line, boundary) = match best {
                Some(best) => best,
                None => self.emergency(start..boundary, line)?,
            };
            return Ok((line, if boundary == end { next } else { boundary }));
        }
        Ok((best.unwrap().0, next))
    }
    fn atom(&mut self, range: Range<usize>) -> Result<(Candidate, usize), Fault> {
        for end in range.start + 1..=range.end {
            let line = self.measure(range.start..end)?;
            if line.width > Scalar::ZERO || end == range.end {
                return Ok((line, end));
            }
        }
        unreachable!("nonempty text range")
    }
    fn emergency(
        &mut self,
        range: Range<usize>,
        line: Candidate,
    ) -> Result<(Candidate, usize), Fault> {
        let mut fallback = (line, range.end);
        for end in (range.start + 1..range.end).rev() {
            let line = self.measure(range.start..end)?;
            if line.width == Scalar::ZERO {
                continue;
            }
            if line.width <= self.limit {
                return Ok((line, end));
            }
            fallback = (line, end);
        }
        Ok(fallback)
    }
    fn offset(&self, index: usize) -> usize {
        self.ranges
            .get(index)
            .map_or(self.paragraph.source.len(), |range| range.start)
    }
    fn measure(&mut self, range: Range<usize>) -> Result<Candidate, Fault> {
        let range = self.offset(range.start)..self.offset(range.end);
        self.remaining = self
            .remaining
            .checked_sub(range.len() + 1)
            .ok_or_else(|| Fault("line reshaping work budget exhausted".into()))?;
        self.paragraph.trace.candidate();
        let mut sequence = self.paragraph.prepared.shape(
            range.clone(),
            &self.paragraph.bidi,
            &self.paragraph.trace,
        )?;
        let source = &self.paragraph.source[range.clone()];
        let kinds = kinds(&mut sequence, source, &self.paragraph.style);
        let mut segment = Paragraph {
            trace: self.paragraph.trace.clone(),
            prepared: self.paragraph.prepared.clone(),
            sequence,
            kinds: kinds.into(),
            style: self.paragraph.style.clone(),
            bidi: self.paragraph.bidi.clone(),
            source: self.paragraph.source.clone(),
            origin: range.start,
            opportunities: Default::default(),
        };
        let mut width = Scalar::ZERO;
        for index in 0..segment.sequence.units.len() {
            let advance = segment.advance(index, width);
            segment.sequence.advance(index, advance);
            width = width + advance;
        }
        let advance = width;
        let width = segment.trim(0..segment.sequence.units.len(), advance).1;
        Ok(Candidate {
            segment,
            width,
            advance,
        })
    }
}
struct Candidate {
    segment: Paragraph,
    width: Scalar,
    advance: Scalar,
}
impl Candidate {
    fn finish(mut self) -> Line {
        let issues = std::mem::take(&mut self.segment.sequence.issues);
        let mut line = self
            .segment
            .finish(0..self.segment.sequence.units.len(), self.advance);
        line.sequence.issues = issues;
        line
    }
}
