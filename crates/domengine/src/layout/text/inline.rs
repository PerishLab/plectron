use super::paragraph::Line;
use crate::layout::{Content, Fragment, measure::View};
use crate::{Fault, Rect};
use graphics::shared::{Glyph, Scalar};
use std::ops::Range;

struct Slice {
    leaf: usize,
    source: Range<usize>,
    edges: [usize; 2],
    clip: Option<Rect>,
}
impl crate::layout::fragment::builder::Builder<'_> {
    pub(super) fn inline(
        &mut self,
        measured: &View<'_>,
        line: &Line,
        parent: usize,
    ) -> Result<(), Fault> {
        let material = measured.material();
        let inline = material.inline().expect("inline provenance");
        let frame = &self.placements[parent];
        let (rect, clip, space) = (frame.rect(), frame.clip, frame.local.space);
        for placed in &line.visual {
            if let Some((index, size)) = line.sequence.atom(placed.unit) {
                let origin = [
                    (Scalar::from(rect.x as i64) + placed.x).ceil(),
                    (Scalar::from(rect.y as i64) + line.baseline - Scalar::from(size[1] as i64))
                        .ceil(),
                ];
                self.emit(
                    &measured
                        .children()
                        .nth(index)
                        .expect("measured inline atom"),
                    origin,
                    (Some(parent), clip),
                )?;
                continue;
            }
            let (part, cluster) = line.sequence.text(placed.unit).expect("text item");
            let index = part
                .run
                .spans
                .partition_point(|span| span.source.end <= cluster.source.start);
            let font = &part.run.spans[index].font;
            let x = Scalar::from(rect.x as i64) + placed.x;
            let glyphs = &part.run.glyphs[cluster.glyphs.clone()];
            let unit = &line.sequence.units[placed.unit];
            let range = line.source.start + unit.source.start..line.source.start + unit.source.end;
            let slices = slices(
                material,
                range,
                [x, x + line.sequence.units[placed.unit].advance],
                placed.level % 2 == 1,
            );
            for slice in slices {
                let leaf = &inline.leaves[slice.leaf];
                self.push(Fragment {
                    id: 0,
                    space,
                    parent: Some(parent),
                    node: Some(leaf.node.id()),
                    target: Some(leaf.target),
                    rect: Rect {
                        x: slice.edges[0],
                        width: slice.edges[1] - slice.edges[0],
                        ..rect
                    },
                    clip,
                    content: Content::Text {
                        font: font.clone(),
                        glyphs: position(
                            glyphs,
                            [
                                x - Scalar::from(slice.edges[0] as i64),
                                line.baseline - font.ascent(),
                            ],
                        )
                        .into(),
                        clusters: vec![slice.source; glyphs.len()].into(),
                        color: leaf.color,
                        clip: slice.clip,
                    },
                })?;
            }
        }
        Ok(())
    }
}
fn position(glyphs: &[textengine::Glyph], origin: [Scalar; 2]) -> Vec<Glyph> {
    let mut x = origin[0];
    glyphs
        .iter()
        .map(|glyph| {
            let placed = Glyph {
                id: glyph.id,
                x: x + glyph.offset[0],
                y: origin[1] + glyph.offset[1],
            };
            x = x + glyph.advance;
            placed
        })
        .collect()
}
fn slices(
    material: &crate::layout::measure::Material,
    source: Range<usize>,
    edges: [Scalar; 2],
    reversed: bool,
) -> Vec<Slice> {
    let leaves = &material.inline().unwrap().leaves;
    let text = &material.text().unwrap()[source.clone()];
    let graphemes = textengine::unicode::graphemes(text);
    let count = |boundary: usize| {
        graphemes.partition_point(|range| range.start < boundary.saturating_sub(source.start))
    };
    let offset = |boundary: usize| {
        let fraction = count(boundary) as f64 / graphemes.len() as f64;
        let fraction = if reversed { 1.0 - fraction } else { fraction };
        (edges[0] + Scalar::new((edges[1] - edges[0]).value() * fraction)).ceil()
    };
    let start = leaves.partition_point(|leaf| leaf.source.end <= source.start);
    let mut result = Vec::new();
    for (index, leaf) in leaves
        .iter()
        .enumerate()
        .skip(start)
        .take_while(|(_, leaf)| leaf.source.start < source.end)
    {
        let a = source.start.max(leaf.source.start);
        let b = source.end.min(leaf.source.end);
        if count(a) == count(b) {
            continue;
        }
        let mut boundaries = [offset(a), offset(b)];
        boundaries.sort();
        let left = if reversed {
            count(b) < graphemes.len()
        } else {
            count(a) > 0
        };
        let right = if reversed {
            count(a) > 0
        } else {
            count(b) < graphemes.len()
        };
        let x = if left { boundaries[0] } else { 0 };
        let end = if right {
            boundaries[1]
        } else {
            super::super::constraints::LIMIT
        };
        result.push(Slice {
            leaf: index,
            source: a - leaf.source.start..b - leaf.source.start,
            edges: boundaries,
            clip: (left || right).then_some(Rect {
                x,
                y: 0,
                width: end.saturating_sub(x),
                height: super::super::constraints::LIMIT,
            }),
        });
    }
    result
}
