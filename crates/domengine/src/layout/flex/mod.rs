use crate::layout::cache::Measurement;
mod item;
pub(in crate::layout) mod request;
use super::input::Entry;
use super::measure::{Measure, Measured};
use super::{Available, Constraints, Shaper};
use crate::{Fault, style::Align};
pub(super) const ITEMS: usize = 256;

impl<S: Shaper> Measure<'_, S> {
    pub fn flex<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
    ) -> Result<Measured<'a>, Fault> {
        let style = &element.key.style;
        let axis = style.flex.direction.axis();
        let cross = 1 - axis;
        let visible: Vec<_> = element.children().collect();
        if visible.len() > self.items {
            return Err(Fault("flex item budget exhausted".into()));
        }
        if self.column(&visible, constraints, &style.flex) {
            return self.segmented(element, constraints);
        }
        let mut lengths = Vec::new();
        let mut inputs = Vec::new();
        let mut bases = Vec::new();
        for child in &visible {
            let align = child.key.style.item.align.unwrap_or(style.flex.align);
            let basis = self.item(*child, (axis, align), constraints.known)?;
            inputs.push(basis.constraints);
            lengths.push(basis.length);
            bases.push(basis.geometry);
        }
        if constraints.known[axis].is_none() && constraints.available[axis] == Available::Min {
            for length in &mut lengths {
                length.minimize();
            }
        }
        let sizes = geoengine::flex(
            &mut lengths,
            constraints.known[axis],
            style.flex.gap,
            super::constraints::LIMIT,
        )
        .map_err(|error| Fault(error.to_string()))?;
        let mut children = Vec::new();
        let mut natural = [0, 0];
        for (((child, input), base), size) in
            visible.iter().zip(inputs.iter_mut()).zip(bases).zip(sizes)
        {
            input.known[axis] = Some(size);
            let measured = self.finish(request::Resolution {
                element: *child,
                constraints: *input,
                geometry: base,
                axis,
                size,
            })?;
            natural[axis] += measured.outer(axis);
            natural[cross] = natural[cross].max(measured.outer(cross));
            children.push(measured);
        }
        natural[axis] += style.flex.gap * children.len().saturating_sub(1);
        let size = [
            style.bound(0, constraints.known[0].unwrap_or(natural[0])),
            style.bound(1, constraints.known[1].unwrap_or(natural[1])),
        ];
        let mut bounded = constraints;
        for dimension in 0..2 {
            if constraints.known[dimension].is_none() && size[dimension] != natural[dimension] {
                bounded.known[dimension] = Some(size[dimension]);
            }
        }
        if bounded.known != constraints.known {
            return self.flex(element, bounded);
        }
        for (measured, input) in children.iter_mut().zip(inputs.iter_mut()) {
            let child = measured.element;
            let align = child.key.style.item.align.unwrap_or(style.flex.align);
            if align.mode == Align::Stretch && child.key.style.preferred(cross).is_none() {
                let stretched = child
                    .key
                    .style
                    .bound(cross, size[cross].saturating_sub(child.key.style.inset()));
                if stretched != measured.size[cross] {
                    input.known[cross] = Some(stretched);
                    *measured = self.geometry(child, *input, request::Operation::Stretch)?;
                }
            }
            measured.offset[cross] =
                align.offset(size[cross] as i64 - measured.outer(cross) as i64)?;
        }
        let free = size[axis] as i64 - natural[axis] as i64;
        let mut offset = style.flex.justify.offset(free)?;
        let count = children.len();
        let mut overflow = [0, 0];
        for (index, child) in children.iter_mut().enumerate() {
            let extra = if style.flex.justify.mode == Align::Between && count > 1 {
                free.max(0) as usize * index / (count - 1)
            } else {
                0
            };
            child.offset[axis] = offset + extra;
            offset += child.outer(axis) + style.flex.gap;
            for (dimension, edge) in overflow.iter_mut().enumerate() {
                *edge = (*edge).max(child.offset[dimension] + child.coverage(dimension));
            }
        }
        Ok(Measured::new(
            element,
            Measurement {
                work: 1,
                pending: false,
                size,
                overflow,
                clipped: constraints.known[1].is_some(),
                children: children.into_iter().map(Measured::placement).collect(),
                text: None,
                quality: geoengine::Quality::Measured,
            },
        ))
    }
}
