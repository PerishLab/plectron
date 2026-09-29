mod envelope;
mod groups;
pub(crate) use groups::Charge;
mod outlines;
mod placement;
mod subtrees;
mod visibility;

#[derive(Default, Debug, Clone, serde::Serialize)]
pub struct Work {
    pub built: u64,
    pub unproven: u64,
    pub subtrees: u64,
    pub bounded: u64,
    pub pruned: u64,
    pub records: u64,
    pub undemanded: u64,
    pub branches: u64,
    pub reused: u64,
    pub measured: u64,
    pub excluded: u64,
    pub skipped: u64,
    pub outlines: u64,
    pub decoded: u64,
    pub paths: u64,
    pub culled: u64,
}
mod raster;
pub use placement::Placement;
pub use raster::{Rasterizer, Report};
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rasterization {
    #[default]
    Direct,
    Strips,
    Local,
}
use crate::shared::{Clip, Evaluation, Fault, Frame, Geometry, Output, Pass, Path, Transform};
use shared::cache::{Cache, Command, Manager, Policy, Snapshot};
use std::sync::Arc;

#[derive(PartialEq, Eq, Hash)]
struct Key {
    evaluation: Evaluation,
    path: Path,
    transform: Transform,
    clip: Clip,
}
pub struct Preparation {
    subtrees: subtrees::Trees,
    groups: groups::Groups,
    visibility: visibility::Visibility,
    outlines: outlines::Outlines,
    work: Work,
    rasterization: Rasterization,
    rasterizer: Rasterizer,
    evaluation: Evaluation,
    manager: Manager,
    cache: Cache<Key, Arc<Geometry>>,
}
impl Default for Preparation {
    fn default() -> Self {
        let manager = Manager::default();
        let cache = manager
            .register("geometry", Policy::bounded(4096, 33_554_432))
            .expect("fresh geometry cache");
        Self {
            subtrees: subtrees::Trees::new(config::Store::default()),
            groups: groups::Groups::new(config::Store::default()),
            visibility: visibility::Visibility::new(config::Store::default()),
            outlines: outlines::Outlines::new(config::Store::default()),
            work: Work::default(),
            manager,
            cache,
            evaluation: Evaluation::Linear,
            rasterization: Rasterization::Direct,
            rasterizer: Rasterizer::default(),
        }
    }
}
impl Preparation {
    pub fn configured(config: config::Store) -> Self {
        Self {
            subtrees: subtrees::Trees::new(config.clone()),
            groups: groups::Groups::new(config.clone()),
            visibility: visibility::Visibility::new(config.clone()),
            outlines: outlines::Outlines::new(config),
            ..Self::default()
        }
    }
    pub(crate) fn grouping(&self) -> bool {
        self.groups.enabled()
    }
    pub(crate) fn omit(&mut self, groups: usize) {
        self.work.undemanded += groups as u64;
    }
    pub fn groups(&self) -> Snapshot {
        self.groups.inspect()
    }
    pub fn pruning(&mut self, command: Command) -> Result<(), Fault> {
        self.groups.command(command)
    }
    pub(crate) fn prune(
        &mut self,
        items: &Arc<[crate::shared::Item]>,
        mapping: &crate::shared::Mapping,
    ) -> Option<Charge> {
        self.work.branches += 1;
        let charge = self.groups.get(items, mapping)?;
        self.work.reused += 1;
        Some(charge)
    }
    pub(crate) fn mark(&self) -> usize {
        self.groups.mark()
    }
    pub(crate) fn empty(
        &mut self,
        items: &Arc<[crate::shared::Item]>,
        mapping: &crate::shared::Mapping,
        context: (Charge, usize),
    ) {
        self.groups.store(items, mapping, context);
    }
    pub fn outlines(&self) -> Snapshot {
        self.outlines.inspect()
    }
    pub fn bounds(&self) -> Snapshot {
        self.visibility.inspect()
    }
    pub fn culling(&mut self, command: Command) -> Result<(), Fault> {
        self.subtrees.clear();
        self.visibility.command(command)
    }
    pub(crate) fn exclude(
        &mut self,
        item: &crate::shared::Item,
        mapping: &crate::shared::Mapping,
        output: Output,
    ) -> Result<Option<usize>, Fault> {
        let crate::shared::Item::Draw {
            origin,
            clip,
            record,
        } = item
        else {
            return Ok(None);
        };
        let crate::shared::Ink::Text { font, glyphs, .. } = record.ink() else {
            return Ok(None);
        };
        if font.source().is_none() {
            return Ok(None);
        }
        if origin.iter().any(|value| *value as u64 > envelope::LIMIT)
            || mapping
                .translation
                .iter()
                .any(|value| value.unsigned_abs() > envelope::LIMIT)
        {
            return Ok(None);
        }
        self.work.records += 1;
        let bounds = self
            .visibility
            .get(record, output, (&mut self.outlines, &mut self.work))?;
        let clip = output.clip(clip.map_or(mapping.clip, |clip| mapping.project(clip)));
        let shift = [
            origin[0] as i64 + mapping.translation[0],
            origin[1] as i64 + mapping.translation[1],
        ];
        if bounds.is_some_and(|bounds| bounds.excludes(shift, output, clip)) {
            self.work.excluded += 1;
            self.work.skipped += glyphs.len() as u64;
            return Ok(Some(glyphs.len()));
        }
        Ok(None)
    }
    pub fn work(&self) -> Work {
        self.work.clone()
    }
    pub(crate) fn outline(
        &mut self,
        font: &crate::shared::Font,
        glyph: u16,
    ) -> Result<Path, Fault> {
        self.work.outlines += 1;
        let (path, decoded) = self.outlines.get(font, glyph)?;
        self.work.decoded += u64::from(decoded);
        Ok(path)
    }
    pub(crate) fn visible(
        &mut self,
        path: &Path,
        transform: Transform,
        clip: Clip,
    ) -> Result<bool, Fault> {
        self.work.paths += 1;
        let hull = placement::bounds(path, transform)?;
        let edges = clip.edges().map(f64::from);
        let visible = (0..2).all(|axis| {
            hull[axis].floor() - 1.0 < edges[axis + 2] && hull[axis + 2].ceil() + 1.0 > edges[axis]
        });
        self.work.culled += u64::from(!visible);
        Ok(visible)
    }
    pub fn rasterization(&mut self, method: Rasterization) {
        self.rasterization = method;
    }
    pub fn rasterizer(&mut self) -> &mut Rasterizer {
        &mut self.rasterizer
    }
    pub(crate) fn primitive(
        &mut self,
        geometry: (Arc<Geometry>, [usize; 2]),
        color: u32,
    ) -> Result<crate::shared::Primitive, Fault> {
        match self.rasterization {
            Rasterization::Direct => Ok(crate::shared::Primitive::Path(geometry.0, color)),
            Rasterization::Strips | Rasterization::Local => Ok(crate::shared::Primitive::Coverage(
                Placement {
                    coverage: self.rasterizer.prepare(&geometry.0)?,
                    offset: geometry.1,
                },
                color,
            )),
        }
    }
    pub fn evaluation(&mut self, evaluation: Evaluation) {
        self.evaluation = evaluation;
    }
    pub fn prepare(&mut self, frame: &Frame, output: Output) -> Result<Pass, Fault> {
        use pipeline::Graph;
        pipeline::step(crate::shared::operation::Prepare { engine: self }).run((frame, output))
    }
    pub(crate) fn build(&mut self, frame: &Frame, output: Output) -> Result<Pass, Fault> {
        self.work = Work::default();
        self.subtrees.maintain()?;
        self.groups
            .begin(frame, output, (self.evaluation, self.rasterization))?;
        self.outlines.maintain()?;
        self.visibility.maintain()?;
        self.cache.maintain();
        let result = Pass::build(frame, output, Some(self));
        self.groups.finish();
        self.cache.publish();
        result
    }
    pub fn inspect(&self) -> Snapshot {
        self.cache.publish();
        self.manager
            .inspect()
            .into_iter()
            .next()
            .expect("owned geometry cache")
    }
    pub fn command(&mut self, command: Command) -> Result<(), Fault> {
        self.manager
            .command(self.cache.id(), command)
            .map_err(|error| Fault(error.to_string()))?;
        self.cache.maintain();
        Ok(())
    }
    pub(crate) fn path(
        &mut self,
        path: Path,
        transform: Transform,
        clip: Clip,
    ) -> Result<(Arc<Geometry>, [usize; 2]), Fault> {
        let (transform, clip, offset) = if self.rasterization == Rasterization::Local {
            placement::normalize(&path, transform, clip)?
        } else {
            (transform, clip, [0; 2])
        };
        let geometry = self.geometry(path, transform, clip)?;
        Ok((geometry, offset))
    }
    fn geometry(
        &mut self,
        path: Path,
        transform: Transform,
        clip: Clip,
    ) -> Result<Arc<Geometry>, Fault> {
        let key = Key {
            evaluation: self.evaluation,
            path,
            transform,
            clip,
        };
        if let Some(geometry) = self.cache.get(&key) {
            return Ok(geometry);
        }
        let geometry = Arc::new(match self.evaluation {
            Evaluation::Linear => Geometry::clipped(&key.path, transform, clip)?,
            Evaluation::Quadratic => Geometry::quadratic(&key.path, transform, clip)?,
        });
        let bytes = std::mem::size_of::<Key>() + key.path.retained() + geometry.retained();
        self.cache.insert(key, geometry.clone(), bytes);
        Ok(geometry)
    }
}
