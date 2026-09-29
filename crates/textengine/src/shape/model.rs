use super::buffer::View;
use crate::{Fault, Font, Scalar};
use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub source: Range<usize>,
    pub glyphs: Range<usize>,
    pub advance: Scalar,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Glyph {
    pub source: Range<usize>,
    pub id: u16,
    pub advance: Scalar,
    pub offset: [Scalar; 2],
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Issue {
    Unmapped(Range<usize>),
    Unpositioned(Range<usize>),
    Lookup {
        table: [u8; 4],
        index: u16,
        kind: u16,
        flags: u16,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shaped {
    pub clusters: Vec<Cluster>,
    pub glyphs: Vec<Glyph>,
    pub issues: Vec<Issue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub spans: Vec<Span>,
    pub height: Scalar,
    pub ascent: Scalar,
    pub clusters: Vec<Cluster>,
    pub glyphs: Vec<Glyph>,
    pub issues: Vec<Issue>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub source: Range<usize>,
    pub glyphs: Range<usize>,
    pub font: Font,
}
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub size: Scalar,
    pub spacing: usize,
    pub script: [u8; 4],
    pub language: Option<[u8; 4]>,
    pub feature: Option<[u8; 4]>,
    pub ligatures: bool,
    pub marks: bool,
}
impl Settings {
    pub fn new(size: Scalar, spacing: usize) -> Self {
        Self {
            size,
            spacing,
            script: *b"DFLT",
            language: None,
            feature: None,
            ligatures: spacing == 0,
            marks: true,
        }
    }
}
impl Run {
    pub(crate) fn append(&mut self, mut shaped: Run, offset: usize) {
        let start = self.glyphs.len();
        let descent = (self.height - self.ascent).max(shaped.height - shaped.ascent);
        self.ascent = self.ascent.max(shaped.ascent);
        self.height = self.ascent + descent;
        for cluster in &mut shaped.clusters {
            shift(&mut cluster.source, offset);
            shift(&mut cluster.glyphs, start);
        }
        for glyph in &mut shaped.glyphs {
            shift(&mut glyph.source, offset);
        }
        for span in &mut shaped.spans {
            shift(&mut span.source, offset);
            shift(&mut span.glyphs, start);
        }
        for issue in &mut shaped.issues {
            match issue {
                Issue::Unmapped(range) | Issue::Unpositioned(range) => shift(range, offset),
                Issue::Lookup { .. } => {}
            }
        }
        self.spans.extend(shaped.spans);
        self.glyphs.extend(shaped.glyphs);
        self.clusters.extend(shaped.clusters);
        self.issues.extend(shaped.issues);
    }
    pub fn validate(&self, source: &str) -> Result<(), Fault> {
        self.structure(source)?;
        let boundaries = crate::unicode::graphemes(source);
        self.boundaries(|end| {
            boundaries
                .binary_search_by_key(&end, |range| range.end)
                .is_ok()
        })
    }
    pub(super) fn check(&self, source: View<'_>) -> Result<(), Fault> {
        self.structure(source.text())?;
        self.boundaries(|end| source.contains(end))
    }
    fn structure(&self, source: &str) -> Result<(), Fault> {
        if !(Scalar::ZERO..=8192.into()).contains(&self.height)
            || self.ascent < Scalar::ZERO
            || self.ascent > self.height
        {
            return Err(Fault("invalid text metrics".into()));
        }
        if self.clusters.len() > 65_536 || self.glyphs.len() > 65_536 || self.issues.len() > 65_536
        {
            return Err(Fault("shaping budget exhausted".into()));
        }
        validate(source, &self.clusters, &self.glyphs)?;
        let mut cursor = 0;
        let mut offset = 0;
        if self.spans.is_empty() || self.spans.len() > 65_536 {
            return Err(Fault("invalid font spans".into()));
        }
        for span in &self.spans {
            span.font.validate()?;
            if span.source.start != offset
                || span.source.end < offset
                || !source.is_char_boundary(span.source.end)
            {
                return Err(Fault("invalid font source mapping".into()));
            }
            offset = span.source.end;
            if span.glyphs.start != cursor
                || span.glyphs.end < cursor
                || span.glyphs.end > self.glyphs.len()
            {
                return Err(Fault("invalid font span mapping".into()));
            }
            for glyph in &self.glyphs[span.glyphs.clone()] {
                span.font.advance(glyph.id)?;
            }
            cursor = span.glyphs.end;
        }
        if cursor != self.glyphs.len() || offset != source.len() {
            return Err(Fault("incomplete font spans".into()));
        }
        Ok(())
    }
    fn boundaries(&self, contains: impl Fn(usize) -> bool) -> Result<(), Fault> {
        for cluster in &self.clusters {
            if !contains(cluster.source.end) {
                return Err(Fault("shaped cluster splits grapheme".into()));
            }
            let index = self
                .spans
                .partition_point(|span| span.source.end <= cluster.source.start);
            if !self.spans.get(index).is_some_and(|span| {
                span.source.end >= cluster.source.end
                    && span.glyphs.start <= cluster.glyphs.start
                    && span.glyphs.end >= cluster.glyphs.end
            }) {
                return Err(Fault("font boundary splits cluster".into()));
            }
        }
        Ok(())
    }
}

impl Cluster {
    fn validate(&self, source: &str, cursor: (usize, usize), count: usize) -> Result<(), Fault> {
        if self.source.start != cursor.0
            || self.source.end <= cursor.0
            || !source.is_char_boundary(self.source.end)
        {
            return Err(Fault("invalid shaped source mapping".into()));
        }
        if self.glyphs.start != cursor.1 || self.glyphs.end < cursor.1 || self.glyphs.end > count {
            return Err(Fault("invalid shaped glyph range".into()));
        }
        self.metric()
    }
    pub(super) fn metric(&self) -> Result<(), Fault> {
        if !(Scalar::ZERO..=8192.into()).contains(&self.advance) {
            return Err(Fault("invalid cluster advance".into()));
        }
        Ok(())
    }
}

impl Glyph {
    pub(super) fn metric(&self) -> Result<(), Fault> {
        if self.advance.value().abs() > 8192.0
            || self
                .offset
                .iter()
                .any(|offset| offset.value().abs() > 8192.0)
        {
            return Err(Fault("invalid shaped glyph metrics".into()));
        }
        Ok(())
    }
    fn validate(&self, source: &str, cluster: &Range<usize>) -> Result<(), Fault> {
        if self.source.start < cluster.start
            || self.source.end > cluster.end
            || self.source.start >= self.source.end
        {
            return Err(Fault("glyph source escapes cluster".into()));
        }
        if !source.is_char_boundary(self.source.start) || !source.is_char_boundary(self.source.end)
        {
            return Err(Fault("invalid glyph source boundary".into()));
        }
        Ok(())
    }
}

impl Shaped {
    pub fn validate(&self, source: &str) -> Result<(), Fault> {
        if self.issues.len() > 65_536 {
            return Err(Fault("shaping diagnostics budget exhausted".into()));
        }
        validate(source, &self.clusters, &self.glyphs)
    }
}
fn validate(source: &str, clusters: &[Cluster], glyphs: &[Glyph]) -> Result<(), Fault> {
    if source.len() > 65_536 || clusters.len() > 65_536 || glyphs.len() > 65_536 {
        return Err(Fault("shaping budget exhausted".into()));
    }
    let mut offset = 0;
    let mut glyph = 0;
    for cluster in clusters {
        cluster.validate(source, (offset, glyph), glyphs.len())?;
        for glyph in &glyphs[cluster.glyphs.clone()] {
            glyph.validate(source, &cluster.source)?;
        }
        offset = cluster.source.end;
        glyph = cluster.glyphs.end;
    }
    if offset != source.len() || glyph != glyphs.len() {
        return Err(Fault("incomplete shaped source mapping".into()));
    }
    for glyph in glyphs {
        glyph.metric()?;
    }
    Ok(())
}

fn shift(range: &mut Range<usize>, offset: usize) {
    range.start += offset;
    range.end += offset;
}
