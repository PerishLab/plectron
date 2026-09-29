use super::{Projection, retention::Retention};
use crate::Fault;
use crate::layout::{
    Constraints, Input, Layout, Shaper,
    measure::{Generation, Impact, Measure, Prediction},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Context {
    pub root: crate::Node,
    pub shaping: u64,
    pub items: usize,
    pub pages: Option<usize>,
}

pub(in crate::layout) struct Assembly<'a> {
    pub geometry: &'a std::sync::Arc<super::Measurement>,
    pub bindings: &'a crate::layout::measure::snapshot::bindings::Bindings,
    pub outer: [usize; 2],
    pub projection: &'a mut Projection,
    pub previous: Option<Previous<'a>>,
    pub demand: Option<&'a [crate::Node]>,
    pub prediction: Option<Prediction>,
    pub audit: bool,
    pub pages: Option<usize>,
}

#[derive(Clone, Copy)]
pub(in crate::layout) struct Previous<'a> {
    pub snapshot: &'a crate::layout::Snapshot,
    pub measurement: &'a Generation,
    pub root: &'a std::sync::Arc<crate::Branch>,
    pub projection: &'a crate::layout::delta::Projection,
}

struct Retained {
    snapshot: std::sync::Arc<crate::layout::Snapshot>,
    measurement: crate::layout::measure::Generation,
    root: std::sync::Arc<crate::Branch>,
    projection: crate::layout::delta::Projection,
}

pub(super) struct Root<'a, 'b, S> {
    pub measure: &'b mut Measure<'a, S>,
    pub previous: Option<&'b Retention>,
    pub retaining: bool,
    pub audit: bool,
    pub pages: Option<usize>,
}

pub(super) struct Evaluation<'a> {
    input: &'a Input,
    constraints: Constraints,
    geometry: std::sync::Arc<super::Measurement>,
    bindings: crate::layout::measure::snapshot::bindings::Bindings,
    outer: [usize; 2],
    previous: Option<Retained>,
    demand: Option<Vec<crate::Node>>,
    prediction: Option<Prediction>,
    audit: bool,
    pages: Option<usize>,
}

impl<'a, S: Shaper> pipeline::Node for Root<'a, '_, S> {
    type Input = (&'a Input, Constraints);
    type Output = Evaluation<'a>;
    type Error = Fault;
    const NAME: &'static str = "layout.measurement";
    fn run(&mut self, (input, constraints): Self::Input) -> Result<Self::Output, Fault> {
        let demand = self
            .measure
            .demand
            .map(|nodes| nodes.iter().copied().collect::<Vec<_>>());
        let cached = self.previous.is_some()
            && self
                .measure
                .memo
                .as_ref()
                .is_some_and(|memo| memo.enabled());
        let reusable = self.retaining && cached;
        let admitted = reusable
            && self.previous.is_some_and(|previous| {
                Impact::admit(&input.tree, constraints, previous.constraints)
            });
        let retained = self
            .previous
            .filter(|_| reusable)
            .map(|previous| {
                previous
                    .bind(
                        input.tree.origin(),
                        constraints,
                        (self.measure.demand, self.measure.estimates.as_deref_mut()),
                        &mut self.measure.bindings,
                    )
                    .map(|measured| measured.map(|measured| (measured, previous.work)))
            })
            .transpose()?
            .flatten();
        let prediction = (self.audit || admitted && retained.is_none()).then(|| {
            Impact::predict(
                &input.tree,
                (constraints, demand.as_deref()),
                self.previous.map(|previous| {
                    (
                        previous.snapshot.tree().as_ref(),
                        previous.constraints,
                        previous.snapshot.demand(),
                    )
                }),
            )
        });
        let measured = if let Some((measured, work)) = retained {
            self.measure.remaining -= work;
            self.measure.memo.as_mut().unwrap().report.retained = 1;
            measured
        } else {
            if admitted
                && let (Some(previous), Some(prediction)) = (self.previous, prediction.as_ref())
            {
                self.measure.prior = Some(crate::layout::measure::Prior::new(
                    previous.snapshot.clone(),
                    previous.measurement.clone(),
                    prediction.publication().clone(),
                ));
            }
            let measured = self.measure.node(input.tree.origin(), constraints)?;
            self.measure.memo.as_mut().unwrap().report.planned = self
                .measure
                .prior
                .as_ref()
                .map_or(0, crate::layout::measure::Prior::planned);
            measured
        };
        Ok(Evaluation {
            input,
            constraints,
            geometry: measured.geometry.clone(),
            bindings: std::mem::take(&mut self.measure.bindings),
            outer: [measured.outer(0), measured.outer(1)],
            previous: self.previous.map(|previous| Retained {
                snapshot: previous.snapshot.clone(),
                measurement: previous.measurement.clone(),
                root: previous.root.clone(),
                projection: previous.projection.clone(),
            }),
            demand,
            prediction,
            audit: self.audit,
            pages: self.pages,
        })
    }
}

pub(super) struct Fragments<'a, 'b> {
    pub projection: &'b mut Projection,
    pub marker: std::marker::PhantomData<&'a Input>,
}
impl<'a> pipeline::Node for Fragments<'a, '_> {
    type Input = Evaluation<'a>;
    type Output = Layout;
    type Error = Fault;
    const NAME: &'static str = "layout.fragments";
    fn run(&mut self, evaluation: Self::Input) -> Result<Layout, Fault> {
        Layout::assemble(
            evaluation.input,
            evaluation.constraints,
            Assembly {
                geometry: &evaluation.geometry,
                bindings: &evaluation.bindings,
                outer: evaluation.outer,
                projection: self.projection,
                previous: evaluation.previous.as_ref().map(|previous| Previous {
                    snapshot: previous.snapshot.as_ref(),
                    measurement: &previous.measurement,
                    root: &previous.root,
                    projection: &previous.projection,
                }),
                demand: evaluation.demand.as_deref(),
                prediction: evaluation.prediction,
                audit: evaluation.audit,
                pages: evaluation.pages,
            },
        )
    }
}

pub(super) struct Timing<'a, O>(pub [u64; 2], pub &'a mut O);
impl<O: pipeline::Observer> pipeline::Observer for Timing<'_, O> {
    fn reset(&mut self) {
        self.1.reset();
    }
    fn begin(&mut self, topology: impl FnOnce() -> pipeline::Topology) {
        self.1.begin(topology);
    }
    fn node<T, E>(
        &mut self,
        identity: (usize, &'static str),
        execute: impl FnOnce() -> Result<T, E>,
    ) -> Result<T, E> {
        let start = std::time::Instant::now();
        let result = self.1.node(identity, execute);
        self.0[identity.0] = start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        result
    }
}
