use super::{Measure, Measured};
use crate::layout::{
    cache::Measurement,
    input::{Entry, Record},
};
use crate::{Constraints, Fault, Shaper};
use pipeline::Graph;

struct Admission<'d, 'a> {
    demand: Option<&'d std::collections::BTreeSet<crate::Node>>,
    estimates: Option<&'d mut crate::layout::cache::Estimates>,
    marker: std::marker::PhantomData<&'a Record>,
}

impl<S: Shaper> Measure<'_, S> {
    pub(super) fn isolate<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
    ) -> Result<Option<Measured<'a>>, Fault> {
        pipeline::step(Admission {
            demand: self.demand,
            estimates: self.estimates.as_deref_mut(),
            marker: std::marker::PhantomData,
        })
        .run((element, constraints))
    }
}
impl<'a> pipeline::Node for Admission<'_, 'a> {
    type Input = (Entry<'a>, Constraints);
    type Output = Option<Measured<'a>>;
    type Error = Fault;
    const NAME: &'static str = "layout.admission";
    fn run(&mut self, (element, constraints): Self::Input) -> Result<Self::Output, Fault> {
        if !element.key.style.independent() {
            return Ok(None);
        }
        if self
            .demand
            .is_none_or(|demand| demand.contains(&element.node))
        {
            return Ok(None);
        }
        let [Some(width), height] = constraints.known else {
            return Err(Fault(
                "isolated layout requires a definite external width".into(),
            ));
        };
        let remembered = self
            .estimates
            .as_deref_mut()
            .and_then(|values| values.get(&element.node));
        let (height, quality) = match remembered {
            Some(estimate) if estimate.matches(element, width) => {
                (estimate.height, estimate.quality)
            }
            Some(estimate) => (estimate.height, geoengine::Quality::Stale),
            None if element.key.style.intrinsic.is_some() && element.key.style.height.is_none() => {
                (
                    element.key.style.intrinsic.unwrap(),
                    geoengine::Quality::Estimated,
                )
            }
            None => (
                height.ok_or_else(|| {
                    Fault("isolated layout requires a height or intrinsic estimate".into())
                })?,
                geoengine::Quality::Measured,
            ),
        };
        let inset = element.key.style.inset();
        if width.saturating_add(inset) > super::super::constraints::LIMIT
            || height.saturating_add(inset) > super::super::constraints::LIMIT
        {
            return Err(Fault("isolated layout size budget exhausted".into()));
        }
        if element.style.overflow == crate::style::Overflow::Auto {
            return Err(Fault(
                "isolated box cannot own a content-dependent scroll range".into(),
            ));
        }
        Ok(Some(Measured::new(
            element,
            Measurement {
                work: 1,
                pending: true,
                size: [width, height],
                overflow: [width, height],
                clipped: true,
                children: Vec::new(),
                text: None,
                quality,
            },
        )))
    }
}
