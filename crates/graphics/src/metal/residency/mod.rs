pub mod pool;
use crate::shared::{Fault, Raster};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_metal::{MTLBuffer, MTLDevice, MTLResource, MTLTexture};
use shared::cache::{Cache, Command, Manager, Policy, Snapshot};
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Residency {
    pub requests: u64,
    pub uploads: u64,
    pub bytes: u64,
}
pub(crate) struct Store {
    pub report: [Residency; 3],
    pub limit: usize,
    pub pool: pool::Pool,
    pub batches: crate::metal::buffer::Batches,
    manager: Manager,
    cache: Cache<u64, Retained<ProtocolObject<dyn MTLTexture>>>,
    coverages: Cache<u64, crate::metal::drawing::coverage::Buffer>,
    paths: Cache<Vec<u64>, crate::metal::buffer::Path>,
}
impl Store {
    pub fn new() -> Self {
        let manager = Manager::default();
        Self {
            report: [Residency::default(); 3],
            limit: 128,
            pool: pool::Pool::default(),
            batches: crate::metal::buffer::Batches::default(),
            coverages: manager
                .register("coverages", Policy::bounded(4096, 33_554_432))
                .expect("fresh coverage residency"),
            paths: manager
                .register("paths", Policy::bounded(4096, 33_554_432))
                .expect("fresh path cache"),
            cache: manager
                .register("textures", Policy::bounded(64, 33_554_432))
                .expect("fresh device cache"),
            manager,
        }
    }
    pub fn texture(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        raster: &Raster,
    ) -> Result<Retained<ProtocolObject<dyn MTLTexture>>, Fault> {
        self.report[0].requests += 1;
        if let Some(texture) = self.cache.get(&raster.id()) {
            return Ok(texture);
        }
        let texture = crate::metal::device::upload(device, raster)?;
        self.report[0].uploads += 1;
        self.report[0].bytes += raster.pixels().len() as u64 * 4;
        self.cache
            .insert(raster.id(), texture.clone(), texture.allocatedSize());
        Ok(texture)
    }
}

impl Store {
    pub fn maintain(&mut self) {
        self.pool.maintain();
        self.cache.maintain();
        self.paths.maintain();
        self.coverages.maintain();
    }
    pub fn publish(&self) {
        self.cache.publish();
        self.paths.publish();
        self.coverages.publish();
    }
    pub fn snapshot(&self, resource: Resource) -> Snapshot {
        self.cache.publish();
        self.paths.publish();
        self.coverages.publish();
        self.manager
            .inspect()
            .into_iter()
            .find(|snapshot| snapshot.id == self.id(resource))
            .expect("owned device cache")
    }
    pub fn command(&mut self, resource: Resource, command: Command) -> Result<(), Fault> {
        self.manager
            .command(self.id(resource), command)
            .map_err(crate::metal::error)?;
        self.cache.maintain();
        self.paths.maintain();
        self.coverages.maintain();
        Ok(())
    }
}

#[derive(Clone, Copy)]
pub enum Resource {
    Textures,
    Paths,
    Coverages,
}
impl Store {
    fn id(&self, resource: Resource) -> u64 {
        match resource {
            Resource::Textures => self.cache.id(),
            Resource::Paths => self.paths.id(),
            Resource::Coverages => self.coverages.id(),
        }
    }
    pub fn path(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        entries: &[crate::metal::buffer::Entry<'_>],
    ) -> Result<crate::metal::buffer::Path, Fault> {
        self.report[1].requests += 1;
        let key: Vec<_> = entries.iter().map(|(coverage, _)| coverage.id()).collect();
        if let Some(path) = self.paths.get(&key) {
            return Ok(path);
        }
        let path = crate::metal::buffer::path(device, entries)?;
        self.report[1].uploads += 1;
        self.report[1].bytes += path.buffer.length() as u64;
        self.paths
            .insert(key, path.clone(), path.buffer.allocatedSize());
        Ok(path)
    }
}

impl Store {
    pub fn coverage(
        &mut self,
        device: &ProtocolObject<dyn MTLDevice>,
        coverage: &crate::shared::Coverage,
    ) -> Result<crate::metal::drawing::coverage::Buffer, Fault> {
        self.report[2].requests += 1;
        if let Some(buffer) = self.coverages.get(&coverage.id()) {
            return Ok(buffer);
        }
        let buffer = crate::metal::drawing::coverage::upload(device, coverage)?;
        self.report[2].uploads += 1;
        self.report[2].bytes += buffer.buffer.length() as u64;
        self.coverages
            .insert(coverage.id(), buffer.clone(), buffer.buffer.allocatedSize());
        Ok(buffer)
    }
}
