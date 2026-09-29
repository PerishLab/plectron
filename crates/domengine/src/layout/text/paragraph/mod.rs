mod kinds;
pub(in crate::layout::text) use kinds::kinds;
mod extraction;
mod metrics;
pub(super) mod sequence;
mod source;
use crate::style::typography::Wrapping;
use crate::{Fault, style::Typography};
use graphics::shared::Scalar;
use sequence::Sequence;
pub(in crate::layout) use source::Source;
use std::{ops::Range, sync::Arc};

#[derive(PartialEq)]
pub(in crate::layout) struct Placed {
    pub unit: usize,
    pub x: Scalar,
    pub level: u8,
}
pub(in crate::layout) struct Line {
    pub visual: Vec<Placed>,
    pub direction: u8,
    pub source: Range<usize>,
    pub sequence: Sequence,
    pub height: usize,
    pub baseline: Scalar,
    pub y: usize,
    pub width: Scalar,
}
pub(in crate::layout) struct Lines {
    accounting: std::sync::OnceLock<shared::cache::Cost>,
    attachment: geoengine::Attachment,
    pub issues: Vec<textengine::Issue>,
    pub descent: Scalar,
    pub ranges: Vec<Line>,
    pub height: usize,
    pub baseline: Scalar,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::layout::text) enum Kind {
    Text,
    Space,
    Tab,
    Break,
}
#[derive(Clone)]
pub(in crate::layout) struct Paragraph {
    pub(in crate::layout::text) trace: crate::layout::cache::Trace,
    pub(in crate::layout::text) sequence: Sequence,
    pub(in crate::layout::text) prepared: Arc<Source>,
    pub(in crate::layout::text) kinds: Arc<[Kind]>,
    pub(in crate::layout::text) style: Typography,
    pub(in crate::layout::text) source: Arc<str>,
    pub(in crate::layout::text) origin: usize,
    pub(in crate::layout::text) bidi: Arc<textengine::bidi::Bidi>,
    pub(in crate::layout::text) opportunities: Arc<[bool]>,
}
impl Paragraph {
    pub fn traced(mut self, trace: crate::layout::cache::Trace) -> Self {
        self.trace = trace;
        self
    }
    pub fn new(
        prepared: Source,
        source: &str,
        style: Typography,
        trace: crate::layout::cache::Trace,
    ) -> Result<Self, Fault> {
        let analyzed = super::breaking::normalize(source, style.whitespace);
        let bidi = textengine::bidi::Bidi::resolve(&analyzed, style.direction)?;
        let mut sequence = prepared.shape(0..source.len(), &bidi, &trace)?;
        let opportunities = super::breaking::opportunities(
            &sequence
                .units
                .iter()
                .map(|cluster| cluster.source.clone())
                .collect::<Vec<_>>(),
            &analyzed,
            &style,
        )?;
        let kinds = kinds(&mut sequence, source, &style);
        Ok(Self {
            trace,
            prepared: Arc::new(prepared),
            sequence,
            kinds: kinds.into(),
            style,
            bidi: Arc::new(bidi),
            source: source.into(),
            origin: 0,
            opportunities: opportunities.into(),
        })
    }
    pub(in crate::layout::text) fn advance(&self, index: usize, width: Scalar) -> Scalar {
        if self.kinds[index] == Kind::Tab {
            let (part, cluster) = self.sequence.text(index).expect("tab is text");
            let font = &part
                .run
                .spans
                .iter()
                .find(|span| span.source.contains(&cluster.source.start))
                .expect("cluster font")
                .font;
            let stop = font.advance(font.index(' ')).expect("validated space") * 8;
            if stop == 0 {
                Scalar::ZERO
            } else {
                Scalar::new(stop.value() - width.value().rem_euclid(stop.value()))
            }
        } else {
            self.sequence.units[index].advance
        }
    }
    pub fn minimum(&mut self) -> Result<usize, Fault> {
        if self.refines() {
            return Ok(self
                .reflow(Scalar::ZERO)?
                .iter()
                .map(|line| line.width.ceil())
                .max()
                .unwrap_or(0));
        }
        let limit = self.style.whitespace.wrap().then_some(Scalar::ZERO);
        let mut maximum = Scalar::ZERO;
        let mut start = 0;
        while start < self.kinds.len() {
            if self.style.whitespace.collapse() && self.kinds[start] == Kind::Space {
                start += 1;
                continue;
            }
            let (range, width, next) = self.line(start, limit);
            maximum = maximum.max(self.trim(range, width).1);
            start = next;
        }
        Ok(maximum.ceil())
    }
    pub fn layout(mut self, limit: Option<usize>, editing: bool) -> Result<Lines, Fault> {
        let height = if self.style.leading == crate::style::typography::Leading::Normal {
            self.sequence.height.ceil()
        } else {
            self.style.leading.height(self.style.font.size)
        };
        if height > 4096 {
            return Err(Fault("line height budget exhausted".into()));
        }
        let baseline =
            (Scalar::from(height as i64) - self.sequence.height) / 2 + self.sequence.ascent;
        let limit = limit
            .filter(|_| !editing && self.style.whitespace.wrap())
            .map(|value| (value as i64).into());
        if let Some(limit) = limit
            && self.refines()
        {
            return Ok(Lines {
                accounting: Default::default(),
                attachment: geoengine::Attachment::fresh(),
                ranges: self.reflow(limit)?,
                issues: self.sequence.issues,
                descent: self.sequence.height - self.sequence.ascent,
                height,
                baseline,
            }
            .position());
        }
        let issues = std::mem::take(&mut self.sequence.issues);
        let mut ranges = Vec::new();
        let mut start = 0;
        while start < self.kinds.len() {
            if self.style.whitespace.collapse() && self.kinds[start] == Kind::Space {
                start += 1;
                continue;
            }
            let (range, width, next) = self.line(start, limit);
            ranges.push(self.finish(range, width));
            start = next;
        }
        if self.kinds.last() == Some(&Kind::Break) || (ranges.is_empty() && editing) {
            ranges.push(Line {
                visual: Vec::new(),
                direction: self
                    .bidi
                    .base(self.bidi.units.last().map_or(0, |unit| unit.source.end)),
                source: self.offset(start)..self.offset(start),
                sequence: self.extract(start..start),
                width: Scalar::ZERO,
                height: 0,
                baseline: Scalar::ZERO,
                y: 0,
            });
        }
        Ok(Lines {
            accounting: Default::default(),
            attachment: geoengine::Attachment::fresh(),
            issues,
            descent: self.sequence.height - self.sequence.ascent,
            ranges,
            height,
            baseline,
        }
        .position())
    }
    fn line(&mut self, start: usize, limit: Option<Scalar>) -> (Range<usize>, Scalar, usize) {
        let mut width = Scalar::ZERO;
        let mut boundary = None;
        for index in start..self.kinds.len() {
            let kind = self.kinds[index];
            if kind == Kind::Break {
                return (start..index, width, index + 1);
            }
            if index > start && width > Scalar::ZERO && self.opportunities[index] {
                boundary = Some((index, width));
            }
            let advance = self.advance(index, width);
            self.sequence.advance(index, advance);
            if advance > Scalar::ZERO
                && width > Scalar::ZERO
                && limit.is_some_and(|limit| {
                    width + advance
                        > limit
                            + Scalar::from(if self.sequence.atom(index).is_none() {
                                self.style.spacing as i64
                            } else {
                                0
                            })
                })
            {
                if let Some((end, measured)) = boundary {
                    return (start..end, measured, end);
                }
                if index > start && self.style.wrapping == Wrapping::Anywhere {
                    return (start..index, width, index);
                }
            }
            width = width + advance;
        }
        (start..self.kinds.len(), width, self.kinds.len())
    }
    pub(in crate::layout::text) fn trim(
        &self,
        mut range: Range<usize>,
        mut width: Scalar,
    ) -> (Range<usize>, Scalar) {
        if self.style.whitespace.collapse() {
            while range.end > range.start && self.kinds[range.end - 1] == Kind::Space {
                range.end -= 1;
                width = width.trim(self.sequence.units[range.end].advance);
            }
        }
        let spacing = if range.end > range.start && self.sequence.atom(range.end - 1).is_none() {
            self.style.spacing
        } else {
            0
        };
        (range, width.trim((spacing as i64).into()))
    }
    pub(in crate::layout::text) fn finish(&self, range: Range<usize>, width: Scalar) -> Line {
        self.trace.line();
        let (range, width) = self.trim(range, width);
        let (mut visual, direction) = self.visual(range.clone());
        for placed in &mut visual {
            placed.unit -= range.start;
        }
        Line {
            visual,
            direction,
            source: self.origin + self.offset(range.start)..self.origin + self.offset(range.end),
            sequence: self.extract(range),
            width,
            height: 0,
            baseline: Scalar::ZERO,
            y: 0,
        }
    }
}
