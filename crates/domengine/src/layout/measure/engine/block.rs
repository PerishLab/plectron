use super::{Measure, Measured};
use crate::Fault;
use crate::layout::cache::Measurement;
use crate::layout::constraints::LIMIT;
use crate::layout::input::Entry;
use crate::layout::{Available, Constraints, Shaper};
use crate::style::Display;

impl<S: Shaper> Measure<'_, S> {
    pub(in crate::layout::measure) fn boxes<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
    ) -> Result<Measured<'a>, Fault> {
        if element.key.style.display == Display::Flex {
            self.operations.flexes += 1;
            return self.flex(element, constraints);
        }
        self.operations.blocks += 1;
        self.block(element, constraints)
    }

    fn block<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
    ) -> Result<Measured<'a>, Fault> {
        let style = &element.key.style;
        let [width, height] = constraints.known;
        let available = [
            width.map_or(constraints.available[0], Available::Definite),
            Available::Max,
        ];
        let mut children = Vec::new();
        for child in element.children() {
            children.push(self.node(
                child,
                Constraints {
                    available,
                    known: [None; 2],
                },
            )?);
        }
        let metrics = children
            .iter()
            .map(|child| {
                let outer = geoengine::Size::positive([child.outer(0), child.outer(1)])?;
                let coverage = geoengine::Size::positive([child.coverage(0), child.coverage(1)])?;
                Ok(geoengine::Child::new(outer, coverage))
            })
            .collect::<Result<Vec<_>, geoengine::Error>>()
            .map_err(|error| Fault(error.to_string()))?;
        let flow = geoengine::Block::new(
            metrics,
            geoengine::Unit::positive(LIMIT).map_err(|error| Fault(error.to_string()))?,
        )
        .resolve()
        .map_err(|error| Fault(error.to_string()))?;
        for (child, offset) in children.iter_mut().zip(flow.placements()) {
            child.offset = offset.array().expect("positive block placement");
        }
        let size = flow.size().array().expect("positive block size");
        let overflow = flow.overflow().array().expect("positive block overflow");
        Ok(Measured::new(
            element,
            Measurement {
                work: 1,
                pending: false,
                size: [
                    style.bound(0, width.unwrap_or(size[0])),
                    style.bound(1, height.unwrap_or(size[1])),
                ],
                overflow,
                clipped: height.is_some(),
                children: children.into_iter().map(Measured::placement).collect(),
                text: None,
                quality: geoengine::Quality::Measured,
            },
        ))
    }
}
