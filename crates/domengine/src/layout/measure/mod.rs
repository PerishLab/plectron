mod demand;
mod engine;
mod image;
mod impact;
pub use impact::{Impact, Survey};
pub(in crate::layout) use impact::{Prediction, Prior};
pub(in crate::layout) mod sequences;
pub use demand::Boundary;
pub(super) mod intrinsic;
mod isolation;
pub mod snapshot;
mod view;
use super::cache::Measurement;
use super::constraints::LIMIT;
use super::input::{Entry, Record};
use super::text::paragraph::Paragraph;
use super::{Available, Constraints, Shaper};
use crate::Fault;
pub(super) use engine::{Measure, Measured};
pub use geoengine::Quality;
pub(in crate::layout) use snapshot::{History, Material};
use std::sync::Arc;
pub(in crate::layout) use view::{Basis, Generation, Source, View};
pub use view::{Metric, State};
impl<S: Shaper> Measure<'_, S> {
    pub fn node<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
    ) -> Result<Measured<'a>, Fault> {
        self.record(element, constraints, (super::cache::Activity::Request, 0));
        self.operations.nodes += 1;
        let before = self.remaining;
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| Fault("layout work budget exhausted".into()))?;
        constraints.check()?;
        let qualitychanged = element.key.style.intrinsic.is_some()
            && self
                .estimates
                .as_deref_mut()
                .and_then(|values| values.get(&element.node))
                .is_some_and(|estimate| estimate.fingerprint != element.key.fingerprint());
        let retained = if !qualitychanged && let Some(prior) = &mut self.prior {
            prior.find(element.id(), &mut self.remaining, &mut self.bindings)?
        } else {
            None
        };
        if let Some(geometry) = retained {
            self.record(
                element,
                constraints,
                (super::cache::Activity::Retain, geometry.work),
            );
            return Ok(geometry.bind(element));
        }
        let mut resolved = self.constraints(&element, constraints);
        if let Some(shell) = self.isolate(element, resolved)? {
            self.record(element, constraints, (super::cache::Activity::Compute, 1));
            return Ok(shell);
        }
        if element.key.style.independent()
            && element.key.style.height.is_none()
            && element.key.style.intrinsic.is_some()
        {
            resolved.known[1] = None;
        }
        let reusable = self
            .memo
            .as_ref()
            .is_some_and(|memo| memo.accepts(&element, self.demand));
        let lookup = if let Some(memo) = &mut self.memo
            && reusable
        {
            Some(memo.find(
                element,
                super::cache::Search {
                    constraints,
                    demand: self.demand,
                    estimates: self.estimates.as_deref_mut(),
                    bindings: &mut self.bindings,
                },
                &mut self.remaining,
            )?)
        } else {
            None
        };
        let cached = lookup.and_then(|lookup| self.reuse(element, constraints, lookup));
        if let Some(measured) = cached {
            return Ok(measured);
        }
        let mut measured = if element.picture.is_some() {
            self.operations.images += 1;
            let input = if element.key.style.independent() {
                resolved
            } else {
                constraints
            };
            self.image(element, input)
        } else if let Some(text) = &element.key.text {
            self.operations.texts += 1;
            self.text(element, text, constraints)?
        } else {
            self.boxes(element, resolved)?
        };
        if element.key.style.independent() {
            let geometry = Arc::make_mut(&mut measured.geometry);
            geometry.clipped = true;
            geometry.overflow = geometry.size;
        }
        if element.edited {
            if constraints.known[1].is_none() && element.key.style.height.is_none() {
                let height = measured
                    .children()
                    .map(|child| child.offset[1] + child.outer(1))
                    .max()
                    .unwrap_or(0);
                Arc::make_mut(&mut measured.geometry).size[1] =
                    element.key.style.bound(1, height + 4);
            }
            Arc::make_mut(&mut measured.geometry).clipped = true;
        }
        if (0..2).any(|axis| measured.outer(axis) > LIMIT || measured.overflow[axis] > LIMIT) {
            return Err(Fault("layout size budget exhausted".into()));
        }
        let work = before - self.remaining;
        Arc::make_mut(&mut measured.geometry).work = work;
        self.record(
            element,
            constraints,
            (super::cache::Activity::Compute, work),
        );
        let disposition = self
            .memo
            .as_mut()
            .filter(|_| reusable)
            .map(|memo| memo.store(&measured, constraints, work));
        if let Some(disposition) = disposition {
            self.admission(element, constraints, disposition, work);
        }
        Ok(measured)
    }

    fn constraints(&self, element: &Record, mut constraints: Constraints) -> Constraints {
        let style = &element.key.style;
        for axis in 0..2 {
            constraints.known[axis] = constraints.known[axis].or(style.preferred(axis));
        }
        if constraints.known[0].is_none() {
            constraints.known[0] = constraints.available[0]
                .definite()
                .map(|width| width.saturating_sub(style.inset()));
        }
        for axis in 0..2 {
            constraints.known[axis] = constraints.known[axis].map(|size| style.bound(axis, size));
        }
        constraints
    }
    fn text<'a>(
        &mut self,
        element: Entry<'a>,
        text: &str,
        constraints: Constraints,
    ) -> Result<Measured<'a>, Fault> {
        let mut style = element.key.style.typography.clone();
        if element.key.editing {
            style.whitespace = crate::style::typography::Whitespace::Pre;
        }
        if style.leading != crate::style::typography::Leading::Normal {
            let height = element
                .key
                .fonts
                .iter()
                .map(|section| style.leading.height(section.font.size))
                .chain(std::iter::once(style.leading.height(style.font.size)))
                .max()
                .unwrap();
            if height > 4096 {
                return Err(Fault("line height budget exhausted".into()));
            }
            style.leading = crate::style::typography::Leading::Pixels(height as u16);
        }
        let mut children = Vec::new();
        for child in element.children() {
            children.push(self.node(
                child,
                Constraints {
                    available: [Available::Max; 2],
                    known: [None; 2],
                },
            )?);
        }
        let sizes: Vec<_> = children
            .iter()
            .map(|child| [child.outer(0), child.outer(1)])
            .collect();
        let identity = (element.key.clone(), sizes.clone());
        let cached = self
            .memo
            .as_mut()
            .and_then(|memo| memo.paragraphs.find(&identity));
        let (mut paragraph, node) = if let Some((paragraph, node)) = cached {
            (paragraph.traced(self.trace.clone()), Some(node))
        } else {
            let prepared = element.prepare(self.shaper, sizes, &self.trace)?;
            let paragraph = Paragraph::new(prepared, text, style.clone(), self.trace.clone())?;
            let node = self
                .memo
                .as_mut()
                .map(|memo| memo.paragraphs.store(identity, &paragraph));
            (paragraph, node)
        };
        let mut limit = constraints.known[0].or(constraints.available[0].definite());
        if limit.is_none() && constraints.available[0] == Available::Min {
            limit = Some(paragraph.minimum()?);
        }
        let lines = if let (Some(memo), Some(node)) = (&self.memo, node) {
            let output = node.run(
                paragraph,
                (limit, element.key.editing),
                memo.paragraphs.access(),
            )?;
            if let super::text::operation::Propagation::Unchanged { baseline } = &output.propagation
            {
                debug_assert!(Arc::ptr_eq(baseline, &output.lines));
            }
            output.lines
        } else {
            Arc::new(paragraph.layout(limit, element.key.editing)?)
        };
        let width = lines
            .ranges
            .iter()
            .map(|line| line.width.ceil())
            .max()
            .unwrap_or(0);
        let size = [width, lines.extent()];
        let container = limit.unwrap_or(width);
        let width = if style.align == crate::style::typography::Align::Start
            && style.direction == textengine::bidi::Direction::Ltr
        {
            constraints.known[0].unwrap_or(width)
        } else {
            container
        };
        let ink = lines.ink();
        let attachment = lines.attachment();
        self.bindings.bind(attachment, lines.clone())?;
        if let Some(memo) = &mut self.memo {
            memo.payloads.store(lines);
        }
        Ok(Measured::new(
            element,
            Measurement {
                work: 1,
                pending: false,
                size: [width, constraints.known[1].unwrap_or(size[1])],
                overflow: [size[0], size[1].max(ink)],
                clipped: false,
                children: children.into_iter().map(Measured::placement).collect(),
                text: Some(attachment),
                quality: geoengine::Quality::Measured,
            },
        ))
    }
}
