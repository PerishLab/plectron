use super::{Entry, Identity, LENGTH, Reference, identity};
use crate::layout::cache::Measurement;
use crate::layout::input::Entry as Node;
use crate::layout::measure::Measure;
use crate::{Constraints, Fault, Shaper};
use shared::cache::Cost;
use std::sync::Arc;

impl<S: Shaper> Measure<'_, S> {
    fn span(&mut self, elements: &[Node<'_>], constraints: Constraints, gap: usize) -> usize {
        let length = LENGTH.min(elements.len());
        let Some(memo) = &mut self.memo else {
            return length;
        };
        if memo.sequences.cache.stats().entries == 0 {
            return length;
        }
        for index in 0..length {
            let suffix = &elements[index..];
            if suffix.len() < LENGTH {
                break;
            }
            if let Some(entry) =
                memo.sequences
                    .cache
                    .lookup(&identity(&suffix[0], constraints, gap))
                && entry
                    .children
                    .first()
                    .and_then(|child| child.key.upgrade())
                    .is_some_and(|key| key == suffix[0].key)
            {
                return if index == 0 { LENGTH } else { index };
            }
        }
        length
    }
    pub fn column(
        &self,
        children: &[Node<'_>],
        constraints: Constraints,
        flex: &crate::style::Flex,
    ) -> bool {
        use crate::style::Align;
        if children.len() < LENGTH
            || !self
                .memo
                .as_ref()
                .is_some_and(|memo| memo.sequences.enabled())
        {
            return false;
        }
        if !constraints.known.iter().all(Option::is_some)
            || flex.direction != crate::style::Direction::Column
            || flex.justify.mode != Align::Start
        {
            return false;
        }
        children.iter().all(|child| {
            let style = &child.key.style;
            if child.key.style.independent() {
                return false;
            }
            style.item.grow() == 0.0
                && style.item.shrink() == 0.0
                && style.item.align.unwrap_or(flex.align).mode == Align::Stretch
        })
    }
    pub fn segmented<'a>(
        &mut self,
        element: Node<'a>,
        constraints: Constraints,
    ) -> Result<crate::layout::measure::Measured<'a>, Fault> {
        let visible: Vec<_> = element.children().collect();
        let gap = element.key.style.flex.gap;
        let mut children = Vec::with_capacity(visible.len());
        let mut size = [0, 0];
        let mut overflow = [0, 0];
        let mut cursor = 0;
        while cursor < visible.len() {
            let remaining = &visible[cursor..];
            let length = self.span(remaining, constraints, gap);
            let elements = &remaining[..length];
            cursor += length;
            if !children.is_empty() {
                size[1] += gap;
            }
            let segment = self.sequence(elements, constraints, gap)?;
            for placement in &segment.children {
                let mut placement = placement.clone();
                placement.offset[1] += size[1];
                children.push(placement);
            }
            overflow[0] = overflow[0].max(segment.overflow[0]);
            overflow[1] = overflow[1].max(size[1] + segment.overflow[1]);
            size[0] = size[0].max(segment.size[0]);
            size[1] += segment.size[1];
            if size[1] > crate::layout::constraints::LIMIT {
                return Err(Fault("layout size budget exhausted".into()));
            }
        }
        Ok(crate::layout::measure::Measured::new(
            element,
            Measurement {
                work: 1,
                pending: false,
                size: std::array::from_fn(|axis| {
                    element
                        .key
                        .style
                        .bound(axis, constraints.known[axis].unwrap_or(size[axis]))
                }),
                overflow,
                clipped: constraints.known[1].is_some(),
                children,
                text: None,
                quality: geoengine::Quality::Measured,
            },
        ))
    }
    pub fn sequence(
        &mut self,
        elements: &[Node<'_>],
        constraints: Constraints,
        gap: usize,
    ) -> Result<Arc<Measurement>, Fault> {
        let identity = (elements.len() == LENGTH).then(|| identity(&elements[0], constraints, gap));
        if let Some(identity) = &identity
            && let Some(memo) = &mut self.memo
            && let Some(entry) = memo.sequences.cache.get(identity)
            && self.remaining >= entry.work
            && memo
                .payloads
                .resolve(&entry.attachments, &mut self.bindings)?
            && let Some(geometry) = entry.bind(elements)
        {
            self.remaining -= entry.work;
            return Ok(geometry);
        }
        let before = self.remaining;
        let mut children = Vec::with_capacity(elements.len());
        let mut size = [0, 0];
        let mut overflow = [0, 0];
        for child in elements {
            if !children.is_empty() {
                size[1] += gap;
            }
            let basis = self.item(
                *child,
                (
                    1,
                    crate::style::Alignment::new(crate::style::Align::Stretch),
                ),
                constraints.known,
            )?;
            let mut input = basis.constraints;
            let length = basis.length;
            input.known[1] = Some(length.bounded());
            let mut measured = self.finish(crate::layout::flex::request::Resolution {
                element: *child,
                constraints: input,
                geometry: basis.geometry,
                axis: 1,
                size: input.known[1].unwrap(),
            })?;
            measured.offset[1] = size[1];
            overflow[0] = overflow[0].max(measured.coverage(0));
            overflow[1] = overflow[1].max(size[1] + measured.coverage(1));
            size[0] = size[0].max(measured.outer(0));
            size[1] += measured.outer(1);
            if size[1] > crate::layout::constraints::LIMIT {
                return Err(Fault("layout size budget exhausted".into()));
            }
            children.push(measured.placement());
        }
        let geometry = Arc::new(Measurement {
            work: before - self.remaining,
            pending: false,
            size,
            overflow,
            clipped: false,
            children,
            text: None,
            quality: geoengine::Quality::Measured,
        });
        if let Some(identity) = identity
            && let Some(memo) = &mut self.memo
            && memo.sequences.cache.writing()
        {
            let attachments = geometry.attachments();
            let cost = Cost::new(
                std::mem::size_of::<Entry>()
                    + std::mem::size_of::<Identity>()
                    + geometry.children.len() * std::mem::size_of::<Reference>()
                    + std::mem::size_of_val(attachments.as_ref()),
            );
            memo.sequences.cache.store(
                identity,
                Entry {
                    size: geometry.size,
                    overflow: geometry.overflow,
                    children: geometry
                        .children
                        .iter()
                        .zip(elements)
                        .map(|(child, element)| Reference {
                            key: Arc::downgrade(&element.key),
                            offset: child.offset,
                            geometry: Arc::downgrade(&child.geometry),
                        })
                        .collect(),
                    attachments,
                    work: before - self.remaining,
                },
                cost,
            );
        }
        Ok(geometry)
    }
}
