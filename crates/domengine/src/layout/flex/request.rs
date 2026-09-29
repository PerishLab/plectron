use super::super::input::Entry;
use super::super::measure::{Measure, Measured};
use super::super::{Constraints, Shaper};
use crate::Fault;
use std::sync::Arc;

pub(in crate::layout) struct Resolution<'a> {
    pub element: Entry<'a>,
    pub constraints: Constraints,
    pub geometry: Option<super::item::Sample<'a>>,
    pub axis: usize,
    pub size: usize,
}

#[derive(Clone, Copy)]
pub(in crate::layout) enum Operation {
    Maximum,
    Narrow,
    Base,
    Minimum,
    Final,
    Stretch,
}

impl<S: Shaper> Measure<'_, S> {
    fn account(&mut self, operation: Operation) {
        let Some(memo) = &mut self.memo else {
            return;
        };
        let requests = &mut memo.report.flex;
        match operation {
            Operation::Maximum => requests.cross.maximum += 1,
            Operation::Narrow => requests.cross.minimum += 1,
            Operation::Base => requests.base += 1,
            Operation::Minimum => requests.minimum += 1,
            Operation::Final => requests.resolved += 1,
            Operation::Stretch => requests.stretch += 1,
        }
    }

    pub(in crate::layout) fn geometry<'a>(
        &mut self,
        element: Entry<'a>,
        constraints: Constraints,
        operation: Operation,
    ) -> Result<Measured<'a>, Fault> {
        self.account(operation);
        self.node(element, constraints)
    }

    pub(in crate::layout) fn bound(
        &mut self,
        element: Entry<'_>,
        axis: usize,
        constraints: Constraints,
        projected: Option<usize>,
    ) -> Result<usize, Fault> {
        self.account(Operation::Minimum);
        if let Some(minimum) = projected {
            if let Some(memo) = &mut self.memo {
                memo.report.flex.projected += 1;
            }
            return Ok(minimum);
        }
        self.minimum(element, axis, constraints)
    }

    pub(in crate::layout) fn finish<'a>(
        &mut self,
        mut resolution: Resolution<'a>,
    ) -> Result<Measured<'a>, Fault> {
        self.account(Operation::Final);
        let eligible = resolution.geometry.as_ref().is_some_and(|sample| {
            sample.measured.size[resolution.axis] == resolution.size
                && self.memo.as_ref().is_some_and(|memo| {
                    memo.stable() && memo.accepts(&sample.measured.element, self.demand)
                })
        });
        if !eligible {
            return self.node(resolution.element, resolution.constraints);
        }
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or_else(|| Fault("layout work budget exhausted".into()))?;
        resolution.constraints.check()?;
        self.record(
            resolution.element,
            resolution.constraints,
            (super::super::cache::Activity::Request, 0),
        );
        let lookup = if let Some(memo) = &mut self.memo {
            Some(memo.find(
                resolution.element,
                super::super::cache::Search {
                    constraints: resolution.constraints,
                    demand: self.demand,
                    estimates: self.estimates.as_deref_mut(),
                    bindings: &mut self.bindings,
                },
                &mut self.remaining,
            )?)
        } else {
            None
        };
        let cached = lookup
            .and_then(|lookup| self.reuse(resolution.element, resolution.constraints, lookup));
        if let Some(measured) = cached {
            return Ok(measured);
        }
        let sample = resolution.geometry.take().unwrap();
        self.remaining = self
            .remaining
            .checked_sub(sample.work - 1)
            .ok_or_else(|| Fault("layout work budget exhausted".into()))?;
        let mut measured = sample.measured;
        Arc::make_mut(&mut measured.geometry).clipped = true;
        let memo = self.memo.as_mut().unwrap();
        memo.report.flex.carried += 1;
        let disposition = memo.store(&measured, resolution.constraints, sample.work);
        self.record(
            resolution.element,
            resolution.constraints,
            (super::super::cache::Activity::Carry, sample.work),
        );
        self.admission(
            resolution.element,
            resolution.constraints,
            disposition,
            sample.work,
        );
        Ok(measured)
    }
}
