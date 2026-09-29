use super::Run;
use crate::{Fault, bidi::Bidi};

impl Run {
    pub fn orient(&mut self, source: &str, bidi: &Bidi) -> Result<(), Fault> {
        self.project(source, bidi, 0)
    }
    pub fn project(&mut self, source: &str, bidi: &Bidi, offset: usize) -> Result<(), Fault> {
        self.validate(source)?;
        offset
            .checked_add(source.len())
            .ok_or_else(|| Fault("bidi source offset overflow".into()))?;
        for cluster in &mut self.clusters {
            let span = self
                .spans
                .partition_point(|span| span.source.end <= cluster.source.start);
            let font = &self.spans[span].font;
            for glyph in &mut self.glyphs[cluster.glyphs.clone()] {
                let index = bidi
                    .units
                    .partition_point(|unit| unit.source.end <= offset + glyph.source.start);
                let unit = bidi
                    .units
                    .get(index)
                    .ok_or_else(|| Fault("missing bidi source mapping".into()))?;
                if unit.level.is_multiple_of(2) {
                    continue;
                }
                let mut characters = source[glyph.source.clone()].chars();
                let character = characters.next().expect("glyph source");
                if characters.next().is_some() {
                    continue;
                }
                let mirrored = crate::bidi::mirror(character);
                if mirrored == character {
                    continue;
                }
                let id = match font.source() {
                    Some(face) => face
                        .index(mirrored)?
                        .ok_or_else(|| Fault("mirrored glyph unavailable".into()))?,
                    None => font.index(mirrored),
                };
                let advance = font.advance(id)?;
                cluster.advance = cluster.advance + advance - glyph.advance;
                glyph.advance = advance;
                glyph.id = id;
            }
        }
        for cluster in &self.clusters {
            cluster.metric()?;
        }
        for glyph in &self.glyphs {
            glyph.metric()?;
        }
        Ok(())
    }
}
