mod admission;
mod nodes;
pub(in crate::layout) use nodes::Assembly;
use pipeline::Graph;
mod retention;
mod trace;
pub use trace::Shaping;
pub(in crate::layout) use trace::Trace;
mod measurement;
mod projection;
pub(in crate::layout) use measurement::{Estimate, Estimates};
pub(in crate::layout) use measurement::{Measurement, Placement};
pub(in crate::layout) use projection::Projection;
mod report;
use super::measure::Measure;
use super::{Constraints, Input, Layout, Shaper};
use crate::{Fault, Node};
pub use report::{
    Activity, Category, Causality, Construction, Estimation, Event, Operations, Reuse,
};
use serde::Deserialize;
mod memo;
mod paragraphs;
pub(in crate::layout) use memo::{Disposition, Lookup, Memo};

pub struct Pipeline<S> {
    demand: Option<std::collections::BTreeSet<Node>>,
    shaper: S,
    intrinsic: super::measure::intrinsic::Intrinsic,
    memo: Memo,
    context: Option<(Node, u64, usize, Option<usize>)>,
    config: config::Store,
    projection: Projection,
    previous: Option<retention::Retention>,
    estimates: Estimates,
}
impl<S: Shaper> Pipeline<S> {
    pub fn new(shaper: S) -> Self {
        Self::managed(shaper, &shared::cache::Manager::default()).expect("fresh cache manager")
    }

    pub fn managed(
        shaper: S,
        manager: &shared::cache::Manager,
    ) -> Result<Self, shared::cache::Fault> {
        Self::configured(shaper, manager, config::Store::default())
    }
    pub fn configured(
        shaper: S,
        manager: &shared::cache::Manager,
        config: config::Store,
    ) -> Result<Self, shared::cache::Fault> {
        Ok(Self {
            demand: None,
            shaper,
            memo: Memo::new(manager, config.clone())?,
            intrinsic: super::measure::intrinsic::Intrinsic::new(manager, config.clone())?,
            config: config.clone(),
            context: None,
            projection: Projection::new(manager, config.clone())?,
            previous: None,
            estimates: Estimates::new(manager, config)?,
        })
    }

    pub fn maintain(&mut self) {
        self.intrinsic.maintain();
        self.memo.maintain();
        self.projection.maintain();
        self.estimates.maintain();
    }
    pub fn close(&mut self) {
        self.intrinsic.close();
        self.memo.close();
        self.projection.close();
        self.estimates.close();
        self.projection.clear();
        self.previous = None;
        self.estimates.clear();
    }

    pub fn build(&mut self, input: &Input, constraints: Constraints) -> Result<Layout, Fault> {
        self.observe(input, constraints, &mut pipeline::Silent)
    }

    pub fn observe(
        &mut self,
        input: &Input,
        constraints: Constraints,
        observer: &mut impl pipeline::Observer,
    ) -> Result<Layout, Fault> {
        self.execute((input, constraints), observer, None)
    }

    pub fn diagnose(
        &mut self,
        input: &Input,
        constraints: Constraints,
        causality: &mut Causality,
    ) -> Result<Layout, Fault> {
        self.execute((input, constraints), &mut pipeline::Silent, Some(causality))
    }

    fn execute(
        &mut self,
        request: (&Input, Constraints),
        observer: &mut impl pipeline::Observer,
        causality: Option<&mut Causality>,
    ) -> Result<Layout, Fault> {
        let (input, constraints) = request;
        observer.reset();
        let retaining = self
            .config
            .get("layout.retention")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.retention: {error}")))?
            .unwrap_or(true);
        let items = self
            .config
            .get("layout.items")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.items: {error}")))?
            .unwrap_or(super::flex::ITEMS);
        let audit = self
            .config
            .get("layout.impact")
            .map(bool::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.impact: {error}")))?
            .unwrap_or(false);
        let pages = self
            .config
            .get("layout.pages")
            .map(usize::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config layout.pages: {error}")))?;
        if let Err(fault) = self
            .intrinsic
            .configure()
            .and_then(|()| self.memo.configure())
            .and_then(|()| self.projection.configure())
            .and_then(|()| self.estimates.configure())
        {
            self.memo.report.revision = input.revision;
            self.memo.report.failure = Some(fault.to_string());
            return Err(fault);
        }
        self.maintain();
        let context = (
            input.tree.element().node,
            self.shaper.revision(),
            items,
            pages,
        );
        let reset = self.context != Some(context);
        if reset {
            self.intrinsic.clear();
            self.memo.clear();
            self.projection.clear();
            self.previous = None;
            self.estimates.clear();
            self.context = Some(context);
        }
        self.memo.report = Reuse {
            reset,
            revision: input.revision,
            enabled: self.memo.enabled(),
            ..Reuse::default()
        };
        let trace = Trace::default();
        let mut measure = Measure {
            demand: self.demand.as_ref(),
            trace: trace.clone(),
            shaper: &self.shaper,
            remaining: 65_536,
            items,
            memo: Some(&mut self.memo),
            intrinsic: Some(&mut self.intrinsic),
            prior: None,
            estimates: Some(&mut self.estimates),
            bindings: Default::default(),
            operations: Default::default(),
            causality,
        };
        let mut timing = nodes::Timing([0; 2], observer);
        let result = pipeline::step(nodes::Root {
            measure: &mut measure,
            previous: self.previous.as_ref(),
            retaining,
            audit,
            pages,
        })
        .then(pipeline::step(nodes::Fragments {
            projection: &mut self.projection,
            marker: std::marker::PhantomData,
        }))
        .observe((input, constraints), &mut timing);
        let work = 65_536 - measure.remaining;
        let operations = measure.operations.clone();
        drop(measure);
        self.memo.report.measurement = timing.0[0];
        self.memo.report.assembly = timing.0[1];
        self.memo.report.charged = work;
        self.memo.report.operations = operations;
        self.memo.report.intrinsic = self.intrinsic.report.clone();
        self.intrinsic.publish();
        self.memo.report.shaping = trace.inspect();
        self.memo.report.fragments = result.as_ref().map_or(0, |layout| layout.root.count);
        self.memo.finish();
        if self.shaper.revision() != context.1 {
            self.intrinsic.clear();
            self.intrinsic.publish();
            self.context = None;
            self.projection.clear();
            self.previous = None;
            self.memo.clear();
            self.memo.finish();
            self.memo.report.fragments = 0;
            self.memo.report.entries = 0;
            self.memo.report.bytes = 0;
            self.memo.report.failure = Some("shaper changed during layout".into());
            return Err(Fault("shaper changed during layout".into()));
        }
        if let Ok(layout) = &result {
            let boundaries = layout.boundaries();
            self.memo.report.isolated = boundaries.len();
            self.memo.report.deferred = boundaries.iter().filter(|b| b.pending).count();
            let mut remembered = boundaries
                .iter()
                .filter(|boundary| boundary.estimated())
                .collect::<Vec<_>>();
            remembered.sort_by_key(|boundary| {
                self.demand
                    .as_ref()
                    .is_some_and(|demand| demand.contains(&boundary.node))
            });
            for boundary in remembered {
                let [width, height] = boundary.content();
                if !boundary.pending
                    && boundary.quality == geoengine::Quality::Measured
                    && let Some(estimate) = self.estimates.get(&boundary.node)
                    && estimate.unresolved(boundary.fingerprint(), width)
                {
                    let error = estimate.height.abs_diff(height);
                    self.memo.report.estimation.resolved += 1;
                    self.memo.report.estimation.error =
                        self.memo.report.estimation.error.saturating_add(error);
                    self.memo.report.estimation.maximum =
                        self.memo.report.estimation.maximum.max(error);
                }
                self.estimates.insert(
                    boundary.node,
                    Estimate {
                        fingerprint: boundary.fingerprint(),
                        width,
                        height,
                        quality: boundary.quality,
                    },
                );
            }
            self.previous = Some(retention::Retention {
                key: input.tree.origin().key.clone(),
                owner: input.tree.origin().record().origin(),
                root: layout.root.clone(),
                measurement: layout.measurement().clone(),
                constraints,
                work,
                snapshot: layout.snapshot.clone(),
                projection: layout.delta.clone(),
            });
            self.memo.report.reused = self.projection.reused;
            self.memo.report.relocated = self.projection.relocated;
        }
        self.memo.report.construction = self.projection.work.clone();
        self.estimates.report(&mut self.memo.report.estimation);
        self.projection.publish();
        self.memo.report.failure = result.as_ref().err().map(ToString::to_string);
        result
    }

    pub fn enable(&mut self, enabled: bool) {
        self.intrinsic.enable(enabled);
        self.memo.enable(enabled);
        self.projection.enable(enabled);
    }

    pub fn inspect(&self) -> Reuse {
        self.memo.report.clone()
    }
    pub fn demand(&mut self, demand: Option<std::collections::BTreeSet<Node>>) -> bool {
        if self.demand == demand {
            return false;
        }
        self.demand = demand;
        self.previous = None;
        true
    }
}
