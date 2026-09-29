use super::paragraph::{
    Line, Placed,
    sequence::{Item, Unit},
};
use crate::{
    Fault,
    layout::{Content, Fragment, measure::View},
};
use graphics::shared::{Font, Glyph};
use std::{ops::Range, sync::Arc};
mod cache;
pub(in crate::layout) use cache::Cache;
mod operation;
mod reuse;

struct Payload {
    font: Font,
    glyphs: Arc<[Glyph]>,
    clusters: Arc<[Range<usize>]>,
}
impl Payload {
    fn bind(&self, color: u32) -> Content {
        Content::Text {
            font: self.font.clone(),
            glyphs: self.glyphs.clone(),
            clusters: self.clusters.clone(),
            clip: None,
            color,
        }
    }
}

impl crate::layout::fragment::builder::Builder<'_> {
    pub(in crate::layout) fn glyphs(
        &mut self,
        measured: &View<'_>,
        index: usize,
        parent: usize,
    ) -> Result<(), Fault> {
        use pipeline::Graph;
        pipeline::step(operation::Glyphs(self)).run((measured, index, parent))
    }
    fn materialize(
        &mut self,
        measured: &View<'_>,
        index: usize,
        parent: usize,
    ) -> Result<(), Fault> {
        let lines = measured.text.as_ref().expect("measured text");
        let line = &lines.ranges[index];
        if measured.material().inline().is_some() {
            return self.inline(measured, line, parent);
        }
        self.projection.work.text.requests += 1;
        if let Some(reuse::Change::Binding(content)) = self.change(measured) {
            self.projection.work.text.retained += 1;
            return self.binding(measured, parent, content);
        }
        let payloads = if let Some(payloads) = self.projection.payloads.get(lines, index) {
            self.projection.work.text.cached += 1;
            payloads
        } else {
            let payloads: Arc<[Payload]> = build(line).into();
            self.projection.work.text.generated +=
                payloads.iter().map(|p| p.glyphs.len()).sum::<usize>();
            self.projection
                .payloads
                .store(lines, index, payloads.clone());
            payloads
        };
        for payload in payloads.iter() {
            self.binding(
                measured,
                parent,
                payload.bind(measured.material().style().color),
            )?;
        }
        Ok(())
    }
    fn binding(
        &mut self,
        measured: &View<'_>,
        parent: usize,
        content: Content,
    ) -> Result<(), Fault> {
        let frame = &self.placements[parent];
        self.push(Fragment {
            id: 0,
            space: frame.local.space,
            parent: Some(parent),
            node: Some(measured.material().node().id()),
            target: Some(measured.material().target()),
            rect: frame.rect(),
            clip: frame.clip,
            content,
        })?;
        Ok(())
    }
}

fn build(line: &Line) -> Vec<Payload> {
    let run = &line.sequence.parts[0].run;
    let units = line.sequence.units.as_slice();
    let mut visual = line.visual.as_slice();
    let mut payloads = Vec::new();
    while !visual.is_empty() {
        let batch = Batch::take(&mut visual, run, units);
        let font = &run.spans[batch.span].font;
        let mut glyphs = Vec::with_capacity(batch.glyphs);
        let mut sources = Vec::with_capacity(batch.glyphs);
        let y = line.baseline - font.ascent();
        for placed in batch.placed {
            let cluster = cluster(run, placed, units);
            let unit = &line.sequence.units[placed.unit];
            let mut x = placed.x;
            for glyph in &run.glyphs[cluster.glyphs.clone()] {
                glyphs.push(Glyph {
                    id: glyph.id,
                    x: x + glyph.offset[0],
                    y: y + glyph.offset[1],
                });
                sources.push(
                    line.source.start + unit.source.start..line.source.start + unit.source.end,
                );
                x = x + glyph.advance;
            }
        }
        if !glyphs.is_empty() {
            payloads.push(Payload {
                font: font.clone(),
                glyphs: glyphs.into(),
                clusters: sources.into(),
            });
        }
    }
    payloads
}

struct Batch<'a> {
    span: usize,
    placed: &'a [Placed],
    glyphs: usize,
}
impl<'a> Batch<'a> {
    fn take(visual: &mut &'a [Placed], run: &textengine::Run, units: &[Unit]) -> Self {
        let index = |placed: &Placed| {
            let source = cluster(run, placed, units).source.start;
            run.spans.partition_point(|span| span.source.end <= source)
        };
        let span = index(&visual[0]);
        let mut count = 0;
        let mut glyphs = 0;
        for placed in visual.iter().take_while(|placed| index(placed) == span) {
            count += 1;
            glyphs += cluster(run, placed, units).glyphs.len();
        }
        let (placed, rest) = visual.split_at(count);
        *visual = rest;
        Self {
            span,
            placed,
            glyphs,
        }
    }
}

fn cluster<'a>(
    run: &'a textengine::Run,
    placed: &Placed,
    units: &[Unit],
) -> &'a textengine::Cluster {
    match units[placed.unit].item {
        Item::Text { part: 0, cluster } => &run.clusters[cluster],
        _ => unreachable!("plain text belongs to one shaped run"),
    }
}
