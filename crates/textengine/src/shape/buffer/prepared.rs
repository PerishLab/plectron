use super::{Buffer, Source, View};
use crate::{Face, Fault, Font, Run, Settings, Span};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selection {
    pub source: Range<usize>,
    pub font: Font,
    pub spacing: usize,
}
pub struct Prepared {
    source: Source,
    parts: Vec<Part>,
}
struct Part {
    source: Range<usize>,
    font: Font,
    settings: Settings,
    buffer: Buffer,
    program: Option<std::sync::Arc<crate::shape::plan::Program>>,
}
impl Prepared {
    pub fn new(source: &str, selections: Vec<Selection>) -> Result<Self, Fault> {
        let (selections, analysis) = validate(source, selections)?;
        let forms = std::cell::OnceCell::new();
        let mut parts = Vec::new();
        for span in &selections {
            let text = &source[span.source.clone()];
            let font = &span.font;
            let settings = font.settings(text, span.spacing);
            let mut buffer = Buffer::new(text, |character| match font.source() {
                Some(face) => face.index(character),
                None => Ok(Some(font.index(character))),
            })?;
            if settings.script == *b"arab" {
                let context = forms.get_or_init(|| {
                    source
                        .char_indices()
                        .zip(crate::unicode::forms(source))
                        .map(|((start, _), form)| (start, form))
                        .collect()
                });
                buffer.joining(text, span.source.start, context);
            }
            if matches!(font.face, Face::Toy | Face::Missing) {
                buffer.issues.extend(
                    text.char_indices()
                        .filter(|(_, character)| {
                            let absent = !character.is_ascii() || font.face == Face::Missing;
                            absent
                                && !character.is_whitespace()
                                && !crate::unicode::hidden(*character)
                        })
                        .map(|(start, character)| {
                            crate::Issue::Unmapped(start..start + character.len_utf8())
                        }),
                );
            }
            parts.push(Part {
                source: span.source.clone(),
                font: font.clone(),
                settings,
                buffer,
                program: None,
            });
        }
        Ok(Prepared {
            source: analysis,
            parts,
        })
    }
}
impl Prepared {
    pub(crate) fn compile(&mut self, compiler: &mut crate::shape::Compiler) -> Result<(), Fault> {
        compiler.configure()?;
        for part in &mut self.parts {
            part.program = compiler.get(&part.font.face, part.settings)?;
        }
        compiler.publish();
        Ok(())
    }
    pub fn cost(&self) -> shared::cache::Cost {
        let mut cost = shared::cache::Cost::new(
            std::mem::size_of::<Self>()
                + self.source.bytes()
                + std::mem::size_of_val(self.parts.as_slice()),
        );
        for part in &self.parts {
            if let Some(program) = &part.program {
                cost.extend(&program.cost());
            }
            cost.bytes += std::mem::size_of_val(part.buffer.slots.as_slice())
                + std::mem::size_of_val(part.buffer.issues.as_slice());
            for slot in &part.buffer.slots {
                cost.bytes += slot.components.bytes();
            }
            if let Some(resource) = part.font.resource() {
                cost.resource(resource);
            }
        }
        cost
    }
    pub fn shape(&self, range: Range<usize>) -> Result<Run, Fault> {
        let view = self.source.view().slice(range.clone())?;
        let index = self
            .parts
            .partition_point(|part| part.source.end <= range.start);
        let mut run: Option<Run> = None;
        for part in self.parts[index..]
            .iter()
            .take_while(|part| part.source.start < range.end)
        {
            let start = range.start.max(part.source.start);
            let end = range.end.min(part.source.end);
            let shaped = part.shape(view.slice(start - range.start..end - range.start)?)?;
            if let Some(run) = &mut run {
                run.append(shaped, start - range.start);
            } else {
                run = Some(shaped);
            }
        }
        let Some(run) = run else {
            let part = &self.parts[index.min(self.parts.len() - 1)];
            return part.shape(view);
        };
        run.check(view)?;
        Ok(run)
    }
}
impl Part {
    fn shape(&self, source: View<'_>) -> Result<Run, Fault> {
        let text = source.text();
        let start = source.origin() - self.source.start;
        let mut buffer = self.buffer.slice(start..start + text.len());
        let mut shaped = if let Some(program) = &self.program {
            program.apply(buffer, self.settings)?
        } else if let Some(face) = self.font.source() {
            face.apply(buffer, self.settings)?
        } else {
            if self.font.face == Face::Toy && self.settings.spacing == 0 {
                buffer.fixture();
            }
            buffer.measure(|glyph| self.font.advance(glyph))?;
            buffer.finish(self.settings.spacing)?
        };
        super::super::grapheme::merge(&mut shaped, source, self.settings.spacing);
        Ok(Run {
            spans: vec![Span {
                source: 0..text.len(),
                glyphs: 0..shaped.glyphs.len(),
                font: self.font.clone(),
            }],
            height: self.font.height(),
            ascent: self.font.ascent(),
            clusters: shaped.clusters,
            glyphs: shaped.glyphs,
            issues: shaped.issues,
        })
    }
}

fn validate(source: &str, selections: Vec<Selection>) -> Result<(Vec<Selection>, Source), Fault> {
    if source.len() > 65_536 || selections.is_empty() || selections.len() > 65_536 {
        return Err(Fault("font selection budget exhausted".into()));
    }
    let analysis = Source::new(source);
    let mut result: Vec<Selection> = Vec::new();
    let mut end = 0;
    for selection in selections {
        selection.font.validate()?;
        let partition = selection.source.start == end && selection.source.end >= end;
        let boundary = analysis.view().contains(selection.source.end);
        let empty = selection.source.is_empty() && !source.is_empty();
        if !partition || !boundary || empty {
            return Err(Fault("font selections must cover source graphemes".into()));
        }
        if selection.spacing > 4096 {
            return Err(Fault("text spacing budget exhausted".into()));
        }
        end = selection.source.end;
        if let Some(previous) = result.last_mut().filter(|previous| {
            previous.font == selection.font && previous.spacing == selection.spacing
        }) {
            previous.source.end = end;
        } else {
            result.push(selection);
        }
    }
    if end != source.len() {
        return Err(Fault("font selections must cover complete source".into()));
    }
    Ok((result, analysis))
}
