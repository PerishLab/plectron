use crate::shared::{Extent, Fault, Layer, Mapping, Rect, Scene};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{
    Arc, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visual {
    pub extent: Extent,
    pub offsets: BTreeMap<u64, usize>,
    pub revision: u64,
}
impl Visual {
    pub fn new(extent: Extent) -> Self {
        Self {
            extent,
            offsets: BTreeMap::new(),
            revision: 0,
        }
    }
    pub fn offset(&self, space: u64) -> usize {
        self.offsets.get(&space).copied().unwrap_or(0)
    }
    fn equivalent(&self, other: &Self) -> bool {
        self.extent == other.extent
            && self
                .offsets
                .iter()
                .filter(|(_, offset)| **offset != 0)
                .eq(other.offsets.iter().filter(|(_, offset)| **offset != 0))
    }
}

#[derive(Debug, Clone)]
pub struct Frame {
    id: u64,
    scene: Scene,
    visual: Visual,
    mappings: Arc<Projection>,
    meter: Arc<Meter>,
}
#[derive(Debug, Default)]
struct Projection(OnceLock<Vec<Mapping>>);

impl Projection {
    fn new(mappings: Vec<Mapping>) -> Self {
        Self(OnceLock::from(mappings))
    }
}
#[derive(Debug, Default)]
struct Meter {
    materializations: AtomicU64,
    spaces: AtomicU64,
    queries: AtomicU64,
    traversals: AtomicU64,
    elapsed: AtomicU64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Materialization {
    pub materializations: u64,
    pub spaces: u64,
    pub queries: u64,
    pub traversals: u64,
    pub elapsed: u64,
}

impl Materialization {
    pub fn since(self, previous: Self) -> Self {
        Self {
            materializations: self.materializations - previous.materializations,
            spaces: self.spaces - previous.spaces,
            queries: self.queries - previous.queries,
            traversals: self.traversals - previous.traversals,
            elapsed: self.elapsed - previous.elapsed,
        }
    }
}

impl Meter {
    fn snapshot(&self) -> Materialization {
        Materialization {
            materializations: self.materializations.load(Ordering::Relaxed),
            spaces: self.spaces.load(Ordering::Relaxed),
            queries: self.queries.load(Ordering::Relaxed),
            traversals: self.traversals.load(Ordering::Relaxed),
            elapsed: self.elapsed.load(Ordering::Relaxed),
        }
    }
}
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Impact {
    pub offsets: usize,
    pub affected: usize,
}
impl Frame {
    pub fn new(scene: Scene, visual: Visual) -> Result<Self, Fault> {
        use pipeline::Graph;
        pipeline::step(crate::shared::operation::Mapping).run((scene, visual))
    }
    pub(crate) fn build(scene: Scene, visual: Visual) -> Result<Self, Fault> {
        Self::eager(scene, visual, Arc::default())
    }
    fn eager(scene: Scene, visual: Visual, meter: Arc<Meter>) -> Result<Self, Fault> {
        let mappings = scene.resolve(&visual)?;
        Ok(Self {
            id: super::identifier()?,
            scene,
            visual,
            mappings: Arc::new(Projection::new(mappings)),
            meter,
        })
    }
    pub fn revise(&self, scene: Scene, visual: Visual) -> Result<Self, Fault> {
        if self.scene.extent == scene.extent
            && Arc::ptr_eq(&self.scene.spaces, &scene.spaces)
            && self.visual.equivalent(&visual)
        {
            return Ok(Self {
                id: super::identifier()?,
                scene,
                visual,
                mappings: self.mappings.clone(),
                meter: self.meter.clone(),
            });
        }
        if Arc::ptr_eq(&self.scene.spaces, &scene.spaces) {
            scene.validate(&visual)?;
            return Ok(Self {
                id: super::identifier()?,
                scene,
                visual,
                mappings: Arc::default(),
                meter: self.meter.clone(),
            });
        }
        Self::eager(scene, visual, self.meter.clone())
    }
    pub fn impact(&self, scene: &Scene, visual: &Visual) -> Impact {
        let mut offsets = Vec::with_capacity(self.visual.offsets.len() + visual.offsets.len());
        offsets.extend(
            self.visual
                .offsets
                .keys()
                .chain(visual.offsets.keys())
                .copied(),
        );
        offsets.sort_unstable();
        offsets.dedup();
        offsets.retain(|id| self.visual.offset(*id) != visual.offset(*id));
        let affected = if self.scene.extent != scene.extent
            || self.visual.extent != visual.extent
            || !Arc::ptr_eq(&self.scene.spaces, &scene.spaces)
        {
            scene.spaces.len()
        } else {
            scene.spaces.affected(&offsets)
        };
        Impact {
            offsets: offsets.len(),
            affected,
        }
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn scene(&self) -> &Scene {
        &self.scene
    }
    pub fn visual(&self) -> &Visual {
        &self.visual
    }
    pub fn extent(&self) -> Extent {
        self.visual.extent
    }
    pub fn mappings(&self) -> &[Mapping] {
        self.resolve(false)
    }
    pub fn projected(&self) -> bool {
        self.mappings.0.get().is_some()
    }
    pub fn spaces(&self) -> usize {
        self.scene.spaces.len()
    }
    pub fn shares(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.mappings, &other.mappings)
    }
    pub fn materialization(&self) -> Materialization {
        self.meter.snapshot()
    }
    pub fn mapping(&self, space: u64) -> Option<&Mapping> {
        self.scene
            .spaces
            .get(space)
            .map(|index| &self.resolve(true)[index])
    }
    pub fn map(&self, space: u64, rect: Rect) -> Option<Rect> {
        self.mapping(space).map(|mapping| mapping.map(rect))
    }
    pub fn prefetch(&self, space: u64, rect: Rect, margin: usize) -> Option<Rect> {
        self.mapping(space)
            .map(|mapping| mapping.prefetch(rect, margin))
    }
    pub fn scroller(&self, space: u64) -> Option<u64> {
        self.scene.spaces.scroller(space)
    }
    pub fn prepare(&self, output: crate::shared::Output) -> Result<crate::shared::Pass, Fault> {
        crate::shared::Pass::new(self, output)
    }
    pub fn plan(&self) -> Result<Layer, Fault> {
        super::plan::build(self, crate::shared::Output::new(self.extent(), 1.0)?, None)
    }
    fn resolve(&self, query: bool) -> &[Mapping] {
        self.mappings
            .0
            .get_or_init(|| {
                let started = Instant::now();
                let mappings = self
                    .scene
                    .resolve(&self.visual)
                    .expect("validated frame projection");
                self.meter.materializations.fetch_add(1, Ordering::Relaxed);
                self.meter
                    .spaces
                    .fetch_add(mappings.len() as u64, Ordering::Relaxed);
                if query {
                    self.meter.queries.fetch_add(1, Ordering::Relaxed);
                } else {
                    self.meter.traversals.fetch_add(1, Ordering::Relaxed);
                }
                self.meter.elapsed.fetch_add(
                    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64,
                    Ordering::Relaxed,
                );
                mappings
            })
            .as_slice()
    }
}
impl Scene {
    pub fn mappings(&self, visual: &Visual) -> Result<Vec<Mapping>, Fault> {
        self.resolve(visual)
    }
    fn resolve(&self, visual: &Visual) -> Result<Vec<Mapping>, Fault> {
        self.validate(visual)?;
        let root = Mapping {
            id: 0,
            parent: None,
            translation: [0, -(visual.offset(0) as i64)],
            clip: Rect {
                x: 0,
                y: 0,
                width: visual.extent.width,
                height: visual.extent.height,
            },
            range: Some(self.extent().height.saturating_sub(visual.extent.height)),
        };
        self.spaces.resolve(root, visual)
    }
    fn validate(&self, visual: &Visual) -> Result<(), Fault> {
        visual.extent.area()?;
        if visual.offsets.values().any(|offset| *offset > 1_048_576) {
            return Err(Fault("visual offset budget exhausted".into()));
        }
        if visual.extent.width != self.extent().width {
            return Err(Fault("visual width differs from scene".into()));
        }
        for (id, offset) in &visual.offsets {
            let range = self.range(*id, visual.extent);
            if range.is_none_or(|range| *offset > range) {
                return Err(Fault("visual state exceeds scene scroll range".into()));
            }
        }
        Ok(())
    }
}
