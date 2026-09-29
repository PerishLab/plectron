use super::super::input::Entry;
use super::super::measure::{Measure, Measured};
use super::super::{Available, Constraints, Shaper};
use crate::{
    Fault,
    style::{Align, Alignment, Display, Overflow},
};

pub(in crate::layout) struct Basis<'a> {
    pub constraints: Constraints,
    pub length: geoengine::Flex,
    pub geometry: Option<Sample<'a>>,
}

pub(in crate::layout) struct Sample<'a> {
    pub measured: Measured<'a>,
    pub work: usize,
}

impl<S: Shaper> Measure<'_, S> {
    pub(in crate::layout) fn item<'a>(
        &mut self,
        element: Entry<'a>,
        context: (usize, Alignment),
        size: [Option<usize>; 2],
    ) -> Result<Basis<'a>, Fault> {
        let (axis, align) = context;
        let cross = 1 - axis;
        let style = &element.key.style;
        let mut constraints = Constraints {
            available: [Available::Max; 2],
            known: [None; 2],
        };
        if let Some(available) = size[cross] {
            constraints.available[cross] = Available::Definite(available);
            if align.mode == Align::Stretch && style.preferred(cross).is_none() {
                constraints.known[cross] =
                    Some(style.bound(cross, available.saturating_sub(style.inset())));
            } else if cross == 0 && style.width.is_none() && style.independent() {
                constraints.known[0] = Some(style.bound(0, 0));
            } else if cross == 0 && style.width.is_none() {
                let maximum = self
                    .geometry(
                        element,
                        Constraints {
                            available: [Available::Max; 2],
                            known: [None; 2],
                        },
                        super::request::Operation::Maximum,
                    )?
                    .size[0];
                let minimum = self
                    .geometry(
                        element,
                        Constraints {
                            available: [Available::Min, Available::Max],
                            known: [None; 2],
                        },
                        super::request::Operation::Narrow,
                    )?
                    .size[0];
                constraints.known[0] = Some(
                    maximum
                        .min(available.saturating_sub(style.inset()))
                        .max(minimum),
                );
            }
        }
        constraints.known[cross] = constraints.known[cross].or(style.preferred(cross));
        constraints.known[axis] = style.item.basis;
        let preferred = style
            .item
            .basis
            .or(style.preferred(axis))
            .or((axis == 1).then_some(style.intrinsic).flatten());
        let (base, projected, geometry) = match preferred {
            Some(base) => (base, None, None),
            None if style.independent() => (0, None, None),
            None => {
                let before = self.remaining;
                let measured =
                    self.geometry(element, constraints, super::request::Operation::Base)?;
                let work = before - self.remaining;
                let eligible = axis == 1 && style.display == Display::Block;
                let minimum = eligible.then_some(measured.size[axis]);
                let base = measured.overflow[axis];
                let sample = eligible.then_some(Sample { measured, work });
                (base, minimum, sample)
            }
        };
        let minimum = match style.minimum[axis] {
            Some(value) => value,
            None if style.overflow != Overflow::Visible => 0,
            None if style.item.shrink() == 0.0 && style.preferred(axis).is_some() => 0,
            None => self
                .bound(element, axis, constraints, projected)?
                .min(style.preferred(axis).unwrap_or(usize::MAX))
                .min(style.maximum[axis].unwrap_or(usize::MAX)),
        };
        Ok(Basis {
            constraints,
            length: geoengine::Flex::new(
                base,
                [
                    minimum,
                    style.maximum[axis]
                        .unwrap_or(super::super::constraints::LIMIT)
                        .max(minimum),
                ],
                [style.item.grow(), style.item.shrink()],
                style.inset(),
            ),
            geometry,
        })
    }
}
