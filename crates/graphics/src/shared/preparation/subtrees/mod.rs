mod admission;
mod cache;
use super::envelope::Envelope;
use crate::shared::{Fault, Ink, Item, Mapping, Output, Preparation};
use admission::{Admission, Route};
pub(super) use cache::Trees;
use pipeline::Graph;
use std::sync::Arc;

impl Preparation {
    pub(crate) fn subtree(
        &mut self,
        items: &Arc<[Item]>,
        mapping: &Mapping,
        context: (Output, usize),
    ) -> Result<bool, Fault> {
        let (output, descendants) = context;
        let envelope = match pipeline::step(Admission(&mut self.subtrees)).run((
            items,
            output,
            descendants,
        ))? {
            Route::Transparent => return Ok(false),
            Route::Retained(envelope) => envelope,
            Route::Build => {
                self.work.built += 1;
                let mut remaining = self.subtrees.items;
                let envelope = self.enclose(items, output, &mut remaining)?;
                self.subtrees.store(items, output, envelope);
                envelope
            }
        };
        self.work.subtrees += 1;
        self.work.unproven += u64::from(envelope.is_none());
        let excluded = envelope.is_some_and(|envelope| {
            envelope.excludes(mapping.translation, output, output.clip(mapping.clip))
        });
        self.work.pruned += u64::from(excluded);
        Ok(excluded)
    }
    pub fn enclosures(&self) -> shared::cache::Snapshot {
        self.subtrees.inspect()
    }
    fn enclose(
        &mut self,
        items: &[Item],
        output: Output,
        remaining: &mut usize,
    ) -> Result<Option<Envelope>, Fault> {
        let mut envelope = Envelope::new();
        for item in items {
            if *remaining == 0 {
                return Ok(None);
            }
            *remaining -= 1;
            self.work.bounded += 1;
            let Some(bounds) = self.enclosure(item, output, remaining)? else {
                return Ok(None);
            };
            envelope.extend(bounds);
        }
        Ok(Some(envelope))
    }
    fn enclosure(
        &mut self,
        item: &Item,
        output: Output,
        remaining: &mut usize,
    ) -> Result<Option<Envelope>, Fault> {
        let bounds = match item {
            Item::Group { opacity: 0, .. } => Some(Envelope::new()),
            Item::Group { space, items, .. } => {
                if space.scroll.is_some() {
                    return Ok(None);
                }
                self.enclose(items, output, remaining)?
                    .and_then(|bounds| bounds.shifted(space.translation, output))
            }
            Item::Fill { color: None, .. } => Some(Envelope::new()),
            Item::Fill { rect, .. } => Envelope::rect(*rect, output),
            Item::Draw { origin, record, .. } => {
                let bounds = self.ink(record, output)?;
                if origin
                    .iter()
                    .any(|value| *value > super::envelope::LIMIT as usize)
                {
                    return Ok(None);
                }
                bounds.and_then(|bounds| bounds.shifted(origin.map(|v| v as i64), output))
            }
            _ => None,
        };
        Ok(bounds)
    }
    fn ink(
        &mut self,
        record: &Arc<crate::shared::Record>,
        output: Output,
    ) -> Result<Option<Envelope>, Fault> {
        Ok(match record.ink() {
            Ink::Fill { color: None, .. } => Some(Envelope::new()),
            Ink::Fill { rect, .. } | Ink::Image { rect, .. } => Envelope::rect(*rect, output),
            Ink::Text { font, .. } if font.source().is_some() => {
                self.visibility
                    .get(record, output, (&mut self.outlines, &mut self.work))?
            }
            _ => None,
        })
    }
}
