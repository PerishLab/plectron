mod components;
mod prepared;
mod source;
use super::{Cluster, Glyph, Issue, Shaped};
use crate::{Fault, Scalar, font::Problem};
pub(super) use components::Components;
pub use prepared::{Prepared, Selection};
pub(super) use source::{Source, View};
use std::ops::Range;

#[derive(Clone)]
pub(super) struct Slot {
    pub source: Range<usize>,
    pub glyph: Glyph,
    pub serial: usize,
    pub components: Components,
    pub form: Option<[u8; 4]>,
    pub hidden: bool,
}
pub(super) struct Buffer {
    pub slots: Vec<Slot>,
    pub issues: Vec<Issue>,
    remaining: usize,
    pub sequence: usize,
}
impl Buffer {
    pub fn new(
        source: &str,
        mapping: impl Fn(char) -> Result<Option<u16>, Problem>,
    ) -> Result<Self, Fault> {
        if source.len() > 65_536 {
            return Err(Fault("text budget exhausted".into()));
        }
        let mut slots = Vec::new();
        let mut issues = Vec::new();
        let mut characters = source.char_indices().peekable();
        while let Some((start, character)) = characters.next() {
            let mut end = start + character.len_utf8();
            if character == '\r' && characters.peek().is_some_and(|(_, next)| *next == '\n') {
                end = characters.next().expect("line ending suffix").0 + 1;
            }
            let id = mapping(
                if character.is_ascii_whitespace() || crate::breaking::mandatory(character) {
                    ' '
                } else {
                    character
                },
            )?;
            if id.is_none() && !crate::unicode::hidden(character) {
                issues.push(Issue::Unmapped(start..end));
            }
            slots.push(Slot {
                source: start..end,
                serial: start,
                components: Components::One(start..end),
                form: None,
                hidden: crate::unicode::hidden(character),
                glyph: Glyph {
                    source: start..end,
                    id: id.unwrap_or(0),
                    advance: Scalar::ZERO,
                    offset: [Scalar::ZERO; 2],
                },
            });
        }
        Ok(Self {
            slots,
            issues,
            remaining: 4_194_304,
            sequence: source.len() + 1,
        })
    }
    pub fn forms(&mut self, source: &str) {
        let forms: std::collections::BTreeMap<_, _> = source
            .char_indices()
            .zip(crate::unicode::forms(source))
            .map(|((start, _), form)| (start, form))
            .collect();
        self.joining(source, 0, &forms);
    }
    pub fn joining(
        &mut self,
        source: &str,
        offset: usize,
        forms: &std::collections::BTreeMap<usize, Option<[u8; 4]>>,
    ) {
        for slot in &mut self.slots {
            slot.form = forms[&(offset + slot.glyph.source.start)];
        }
        for range in crate::unicode::graphemes(source) {
            let start = self
                .slots
                .partition_point(|slot| slot.source.end <= range.start);
            let end = self
                .slots
                .partition_point(|slot| slot.source.start < range.end);
            for slot in &mut self.slots[start..end] {
                slot.source = range.clone();
            }
        }
        let character = |slot: &Slot| source[slot.glyph.source.clone()].chars().next().unwrap();
        let mut start = 0;
        while start < self.slots.len() {
            let end = self.slots[start + 1..]
                .iter()
                .position(|slot| crate::unicode::combining(character(slot)) == 0)
                .map_or(self.slots.len(), |offset| start + 1 + offset);
            let first =
                start + usize::from(crate::unicode::combining(character(&self.slots[start])) == 0);
            self.slots[first..end].sort_by_key(|slot| crate::unicode::order(character(slot)));
            start = end;
        }
    }
    pub fn slice(&self, range: Range<usize>) -> Self {
        let start = self
            .slots
            .partition_point(|slot| slot.source.end <= range.start);
        let end = self
            .slots
            .partition_point(|slot| slot.source.start < range.end);
        let mut slots = self.slots[start..end].to_vec();
        for slot in &mut slots {
            slot.source = shift(&slot.source, range.start);
            slot.glyph.source = shift(&slot.glyph.source, range.start);
            for component in slot.components.edit() {
                *component = shift(component, range.start);
            }
        }
        let first = self.issues.partition_point(
            |issue| matches!(issue, Issue::Unmapped(source) if source.end <= range.start),
        );
        let issues = self.issues[first..]
            .iter()
            .take_while(
                |issue| matches!(issue, Issue::Unmapped(source) if source.start < range.end),
            )
            .filter_map(|issue| match issue {
                Issue::Unmapped(source)
                    if source.start >= range.start && source.end <= range.end =>
                {
                    Some(Issue::Unmapped(shift(source, range.start)))
                }
                _ => None,
            })
            .collect();
        Self {
            slots,
            issues,
            remaining: self.remaining,
            sequence: self.sequence,
        }
    }
    pub fn charge(&mut self, count: usize) -> Result<(), Problem> {
        self.remaining = self.remaining.checked_sub(count).ok_or(Problem::Budget)?;
        Ok(())
    }
    pub fn measure(&mut self, advance: impl Fn(u16) -> Result<Scalar, Fault>) -> Result<(), Fault> {
        for slot in &mut self.slots {
            slot.glyph.advance = if slot.hidden {
                Scalar::ZERO
            } else {
                advance(slot.glyph.id)?
            };
        }
        Ok(())
    }
    pub fn fixture(&mut self) {
        self.slots.dedup_by(|next, slot| {
            if slot.glyph.id != u16::from(b'f') || next.glyph.id != u16::from(b'i') {
                return false;
            }
            slot.source.end = next.source.end;
            slot.glyph.id = 128;
            slot.glyph.source = slot.source.clone();
            true
        });
    }
    pub fn finish(self, spacing: usize) -> Result<Shaped, Fault> {
        if spacing > 4096 {
            return Err(Fault("text spacing budget exhausted".into()));
        }
        let mut clusters: Vec<Cluster> = Vec::new();
        let mut glyphs = Vec::new();
        for slot in self.slots {
            let index = glyphs.len();
            let count = usize::from(!slot.hidden);
            if let Some(cluster) = clusters
                .last_mut()
                .filter(|cluster| cluster.source == slot.source)
            {
                cluster.glyphs.end = index + count;
                cluster.advance = cluster.advance + slot.glyph.advance;
            } else {
                clusters.push(Cluster {
                    source: slot.source,
                    glyphs: index..index + count,
                    advance: slot.glyph.advance + ((spacing * count) as i64).into(),
                });
            }
            if !slot.hidden {
                glyphs.push(slot.glyph);
            }
        }
        Ok(Shaped {
            clusters,
            glyphs,
            issues: self.issues,
        })
    }
}

fn shift(range: &Range<usize>, offset: usize) -> Range<usize> {
    range.start - offset..range.end - offset
}
