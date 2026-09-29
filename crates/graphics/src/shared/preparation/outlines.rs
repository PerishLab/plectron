use crate::shared::{Face, Fault, Font, Path};
use serde::Deserialize;
use shared::cache::{Cache, Manager, Policy, Snapshot};

const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 512,
    bytes: 4_194_304,
};

pub(super) struct Outlines {
    config: config::Store,
    manager: Manager,
    cache: Cache<(u64, u16), Path>,
}
impl Outlines {
    pub fn new(config: config::Store) -> Self {
        let manager = Manager::default();
        let cache = manager
            .register("outlines", POLICY)
            .expect("fresh outline cache");
        Self {
            config,
            manager,
            cache,
        }
    }
    pub fn maintain(&mut self) -> Result<(), Fault> {
        let policy = self
            .config
            .get("render.outlines")
            .map(Policy::deserialize)
            .transpose()
            .map_err(|error| Fault(format!("config render.outlines: {error}")))?
            .unwrap_or(POLICY);
        if policy != self.cache.policy() {
            self.cache
                .configure(policy)
                .map_err(|error| Fault(error.to_string()))?;
        }
        Ok(())
    }
    pub fn inspect(&self) -> Snapshot {
        self.cache.publish();
        self.manager
            .inspect()
            .into_iter()
            .next()
            .expect("owned outline cache")
    }
    pub fn get(&mut self, font: &Font, glyph: u16) -> Result<(Path, bool), Fault> {
        let Face::Owned(lease) = &font.face else {
            return Ok((font.outline(glyph)?, true));
        };
        let key = (lease.id(), glyph);
        if let Some(path) = self.cache.get(&key) {
            return Ok((path, false));
        }
        let path = font.outline(glyph)?;
        let bytes = path.retained() + std::mem::size_of::<Path>() + std::mem::size_of_val(&key);
        self.cache.insert(key, path.clone(), bytes);
        Ok((path, true))
    }
}
