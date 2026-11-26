use platform::shared::fonts::{Discovery, Fonts, Tone};
use std::collections::BTreeMap;
use std::sync::Mutex;
use textengine::font::Lease;
use textengine::{Face, Fault, Resolver, Weight};

#[derive(Default)]
pub struct System {
    host: platform::fonts::System,
    cache: Mutex<BTreeMap<(String, Weight), Option<Face>>>,
}

impl System {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Resolver for System {
    fn resolve(&self, family: &str, weight: Weight) -> Result<Option<Face>, Fault> {
        let key = (family.to_lowercase(), weight);
        if let Some(cached) = self.cache.lock().expect("system font cache lock").get(&key) {
            return Ok(cached.clone());
        }
        let tone = match weight {
            Weight::Normal => Tone::Normal,
            Weight::Bold => Tone::Bold,
        };
        let discovered = self.host.discover(family, tone).map_err(Fault)?;
        let face = match discovered {
            Discovery::Found(asset) => Some(Face::Owned(Lease::face(asset.bytes, asset.index)?)),
            Discovery::Absent | Discovery::Unsupported => None,
        };
        let mut cache = self.cache.lock().expect("system font cache lock");
        if cache.len() < 128 {
            cache.insert(key, face.clone());
        }
        Ok(face)
    }
}
