use crate::shared::{Extent, Fault};
use serde::{Deserialize, Serialize};
use std::{
    hash::{Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
static SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Serialize)]
#[serde(into = "Pixels")]
pub struct Raster {
    id: u64,
    extent: Extent,
    pixels: Arc<[u32]>,
}
#[derive(Serialize, Deserialize)]
struct Pixels {
    extent: Extent,
    pixels: Vec<u32>,
}
impl Raster {
    pub fn rgba(extent: Extent, bytes: &[u8]) -> Result<Self, Fault> {
        if extent.area()? > 1_048_576 || bytes.len() != extent.area()? * 4 {
            return Err(Fault("invalid RGBA byte count".into()));
        }
        let pixels = bytes
            .chunks_exact(4)
            .map(|rgba| {
                let alpha = u32::from(rgba[3]);
                let channel = |index: usize| (u32::from(rgba[index]) * alpha + 127) / 255;
                alpha << 24 | channel(0) << 16 | channel(1) << 8 | channel(2)
            })
            .collect();
        Self::admit(Pixels { extent, pixels })
    }
    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn extent(&self) -> Extent {
        self.extent
    }
    pub fn pixels(&self) -> &[u32] {
        &self.pixels
    }
    pub fn retained(&self) -> usize {
        std::mem::size_of::<Self>() + self.pixels.len() * 4
    }
}
impl Raster {
    fn admit(value: Pixels) -> Result<Self, Fault> {
        if value.extent.area()? > 1_048_576 || value.pixels.len() != value.extent.area()? {
            return Err(Fault("raster pixel budget exhausted".into()));
        }
        if value.pixels.iter().any(|pixel| {
            [0, 8, 16]
                .iter()
                .any(|shift| ((pixel >> shift) & 255) > pixel >> 24)
        }) {
            return Err(Fault("raster requires premultiplied RGBA".into()));
        }
        let id = SEQUENCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| Fault("raster identities exhausted".into()))?;
        Ok(Self {
            id,
            extent: value.extent,
            pixels: value.pixels.into(),
        })
    }
}
impl From<Raster> for Pixels {
    fn from(value: Raster) -> Self {
        Self {
            extent: value.extent,
            pixels: value.pixels.to_vec(),
        }
    }
}
impl PartialEq for Raster {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}
impl Eq for Raster {}
impl Hash for Raster {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<'de> Deserialize<'de> for Raster {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::admit(Pixels::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}
