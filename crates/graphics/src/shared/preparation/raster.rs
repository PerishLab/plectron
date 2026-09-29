use crate::shared::{Coverage, Fault, Geometry};
use shared::cache::{Cache, Command, Manager, Policy, Snapshot};
use std::{sync::Arc, time::Instant};

#[derive(Debug, Default, Clone, Copy)]
pub struct Report {
    pub builds: u64,
    pub generation: u64,
    pub visits: u64,
    pub crossings: u64,
}
pub struct Rasterizer {
    cache: Cache<u64, Arc<Coverage>>,
    manager: Manager,
    report: Report,
}
impl Default for Rasterizer {
    fn default() -> Self {
        let manager = Manager::default();
        let cache = manager
            .register("coverage", Policy::bounded(4096, 33_554_432))
            .expect("fresh coverage cache");
        Self {
            manager,
            cache,
            report: Report::default(),
        }
    }
}
impl Rasterizer {
    pub fn prepare(&mut self, geometry: &Geometry) -> Result<Arc<Coverage>, Fault> {
        self.cache.maintain();
        if let Some(coverage) = self.cache.get(&geometry.id()) {
            return Ok(coverage);
        }
        let start = Instant::now();
        let coverage = Arc::new(Coverage::new(geometry)?);
        self.report.builds += 1;
        self.report.generation += start.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64;
        self.report.visits += coverage.work().visits;
        self.report.crossings += coverage.work().crossings;
        self.cache
            .insert(geometry.id(), coverage.clone(), coverage.retained());
        Ok(coverage)
    }
    pub fn inspect(&self) -> Snapshot {
        self.cache.publish();
        self.manager
            .inspect()
            .into_iter()
            .next()
            .expect("owned coverage cache")
    }
    pub fn report(&self) -> Report {
        self.report
    }
    pub fn command(&mut self, command: Command) -> Result<(), Fault> {
        self.manager
            .command(self.cache.id(), command)
            .map_err(|error| Fault(error.to_string()))?;
        self.cache.maintain();
        Ok(())
    }
}
