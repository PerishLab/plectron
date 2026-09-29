use crate::{Extent, Fault};
use measure::{Generation, Measure};
use std::sync::{Arc, OnceLock};

mod cache;
mod constraints;
pub mod delta;
pub use cache::{
    Activity, Category, Causality, Construction, Estimation, Event, Operations, Pipeline, Reuse,
    Shaping,
};
pub use paint::operation;
mod flex;
mod fragment;
mod input;
mod measure;
mod paint;
pub use paint::{Painter, Painting};
mod text;
pub use constraints::{Available, Constraints};
pub use fragment::{Branch, Content, Coverage, Fragment, Link, Local, Piece, Placement};
pub use input::{
    Capture, Cause, Edge, Entity, Id, Input, Mode, Plan, Relation, Role, Tree, Update,
};
pub use measure::intrinsic::Report as Intrinsic;
pub use measure::snapshot::{Collection, Snapshot};
pub use measure::snapshot::{Lifetime, Liveness, Memory, References, Residency};
pub use measure::{Boundary, Impact, Metric, Quality, State, Survey};
pub use text::{Cluster, Run, Shaper, Text};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bridge {
    Measure,
    Fragment,
    Space,
    Paint,
    Hit,
    Effect,
}
impl Bridge {
    const ALL: [Self; 6] = [
        Self::Measure,
        Self::Fragment,
        Self::Space,
        Self::Paint,
        Self::Hit,
        Self::Effect,
    ];
    const fn slot(self) -> usize {
        self as usize
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Usage {
    calls: [u64; 6],
}
impl Usage {
    pub fn calls(&self, bridge: Bridge) -> u64 {
        self.calls[bridge.slot()]
    }
    pub fn active(&self) -> impl Iterator<Item = Bridge> + '_ {
        Bridge::ALL
            .into_iter()
            .filter(|bridge| self.calls(*bridge) > 0)
    }
}
#[derive(Debug)]
struct Ledger([std::sync::atomic::AtomicU64; 6]);
impl Ledger {
    fn new() -> Arc<Self> {
        let ledger = Arc::new(Self(std::array::from_fn(|_| {
            std::sync::atomic::AtomicU64::new(0)
        })));
        ledger.add(Bridge::Measure);
        ledger.add(Bridge::Fragment);
        ledger
    }
    fn add(&self, bridge: Bridge) {
        self.0[bridge.slot()].fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }
    fn inspect(&self) -> Usage {
        Usage {
            calls: std::array::from_fn(|slot| {
                self.0[slot].load(std::sync::atomic::Ordering::Relaxed)
            }),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Layout {
    revision: u64,
    fragments: Arc<OnceLock<Arc<[Fragment]>>>,
    placements: Arc<OnceLock<Vec<Placement>>>,
    root: Arc<Branch>,
    delta: delta::Projection,
    extent: Extent,
    snapshot: Arc<Snapshot>,
    measurement: Generation,
    ledger: Arc<Ledger>,
}

impl Layout {
    pub fn build(
        input: &Input,
        constraints: Constraints,
        shaper: &impl Shaper,
    ) -> Result<Self, Fault> {
        let mut measure = Measure {
            demand: None,
            trace: cache::Trace::default(),
            shaper,
            remaining: 65_536,
            items: flex::ITEMS,
            memo: None,
            intrinsic: None,
            prior: None,
            estimates: None,
            bindings: Default::default(),
            operations: Default::default(),
            causality: None,
        };
        let measured = measure.node(input.tree.origin(), constraints)?;
        Self::assemble(
            input,
            constraints,
            cache::Assembly {
                geometry: &measured.geometry,
                bindings: &measure.bindings,
                outer: [measured.outer(0), measured.outer(1)],
                projection: &mut cache::Projection::temporary(),
                previous: None,
                demand: None,
                prediction: None,
                audit: false,
                pages: None,
            },
        )
    }
    fn assemble(
        input: &Input,
        constraints: Constraints,
        context: cache::Assembly<'_>,
    ) -> Result<Self, Fault> {
        let cache::Assembly {
            geometry,
            bindings,
            outer,
            projection,
            previous,
            demand,
            prediction,
            audit,
            pages,
        } = context;
        let (snapshot, measurement) = Snapshot::build(
            input.tree.clone(),
            geometry,
            bindings,
            (
                constraints,
                demand,
                previous.map(|value| (value.0, value.1)),
                prediction,
                audit,
                pages,
            ),
        )?;
        let root = projection.build(
            &snapshot.view(),
            (None, [0; 2]),
            previous.map(|value| value.2),
        )?;
        let delta = delta::Projection::new(&root, previous.map(|value| value.3));
        Ok(Self {
            revision: input.revision,
            fragments: Arc::default(),
            placements: Arc::default(),
            root,
            delta,
            extent: Extent {
                width: constraints.available[0]
                    .definite()
                    .unwrap_or(0)
                    .max(outer[0]),
                height: constraints.available[1]
                    .definite()
                    .unwrap_or(0)
                    .max(outer[1]),
            },
            snapshot,
            measurement,
            ledger: Ledger::new(),
        })
    }
    pub fn root(&self) -> &Arc<Branch> {
        &self.root
    }
    pub fn delta(&self) -> &delta::Projection {
        &self.delta
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn tree(&self) -> &Arc<Tree> {
        self.snapshot.tree()
    }
    pub fn usage(&self) -> Usage {
        self.ledger.inspect()
    }
    pub fn residency(&self) -> Residency {
        Residency::inspect(self)
    }
    pub fn lifetime(&self) -> Lifetime {
        Lifetime::new(self)
    }
    pub(in crate::layout) fn bridge(&self, bridge: Bridge) {
        self.ledger.add(bridge);
    }
    pub fn fragments(&self) -> &[Fragment] {
        self.records()
    }
    pub fn roots(&self, visit: &mut dyn FnMut(crate::Node)) {
        self.visit(|placement| {
            if let Some(node) = placement.local.target {
                visit(node);
            }
        });
    }
    pub fn records(&self) -> &Arc<[Fragment]> {
        self.fragments.get_or_init(|| {
            let mut fragments = Vec::with_capacity(self.root.count());
            self.visit(|placement| fragments.push(placement.resolve()));
            fragments.into()
        })
    }
    pub fn snapshot(&self) -> &Arc<Snapshot> {
        &self.snapshot
    }
    pub(in crate::layout) fn measurement(&self) -> &Generation {
        &self.measurement
    }
    pub fn extent(&self) -> Extent {
        self.extent
    }
    pub fn effects(
        &self,
        changes: &[crate::style::animation::Change],
    ) -> Vec<graphics::shared::Effect> {
        self.bridge(Bridge::Effect);
        let changes = changes
            .iter()
            .map(|change| (change.node.id(), change.opacity))
            .collect::<std::collections::BTreeMap<_, _>>();
        self.fragments()
            .iter()
            .filter_map(|fragment| {
                let node = fragment.node?;
                let opacity = changes.get(&node)?;
                matches!(fragment.content, Content::Box { .. }).then_some(
                    graphics::shared::Effect {
                        space: fragment.space,
                        opacity: *opacity,
                    },
                )
            })
            .collect()
    }
    pub fn tracks(
        &self,
        tracks: &[crate::style::animation::Track<crate::Node>],
    ) -> Vec<crate::style::animation::Track<u64>> {
        self.bridge(Bridge::Effect);
        let tracks = tracks
            .iter()
            .map(|track| (track.target.id(), &track.plan))
            .collect::<std::collections::BTreeMap<_, _>>();
        self.fragments()
            .iter()
            .filter_map(|fragment| {
                let node = fragment.node?;
                let plan = tracks.get(&node)?;
                matches!(fragment.content, Content::Box { .. }).then_some(
                    crate::style::animation::Track {
                        target: fragment.space,
                        plan: (*plan).clone(),
                    },
                )
            })
            .collect()
    }
    pub fn hit(&self, x: usize, y: usize) -> Option<crate::Node> {
        self.bridge(Bridge::Hit);
        self.root
            .find(self.scope(), &mut |_| true, &mut |_, rect| {
                rect.contains(x, y)
            })
            .and_then(|local| local.target)
    }
}
