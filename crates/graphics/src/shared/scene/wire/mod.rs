mod paint;
use crate::shared::{Fault, Raster, Scalar, Weight, font};
pub use paint::{Ink, Item};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Resources {
    fonts: BTreeMap<u64, font::Lease>,
    rasters: BTreeMap<u64, Raster>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Face {
    Toy,
    Missing,
    Owned(u64),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Font {
    pub face: Face,
    pub size: Scalar,
    pub weight: Weight,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub fonts: BTreeMap<u64, usize>,
    pub rasters: BTreeMap<u64, usize>,
}
impl Resources {
    pub fn font(&mut self, font: crate::shared::Font) -> Font {
        let face = match font.face {
            crate::shared::Face::Toy => Face::Toy,
            crate::shared::Face::Missing => Face::Missing,
            crate::shared::Face::Owned(lease) => {
                let id = lease.id();
                self.fonts.entry(id).or_insert(lease);
                Face::Owned(id)
            }
        };
        Font {
            face,
            size: font.size,
            weight: font.weight,
        }
    }
    pub fn raster(&mut self, raster: Raster) -> u64 {
        let id = raster.id();
        self.rasters.entry(id).or_insert(raster);
        id
    }
    pub fn manifest(&self) -> Manifest {
        Manifest {
            fonts: self
                .fonts
                .iter()
                .map(|(id, lease)| (*id, lease.length()))
                .collect(),
            rasters: self
                .rasters
                .iter()
                .map(|(id, raster)| (*id, std::mem::size_of_val(raster.pixels())))
                .collect(),
        }
    }
    pub fn resolve(&self, font: Font) -> Result<crate::shared::Font, Fault> {
        let face = match font.face {
            Face::Toy => crate::shared::Face::Toy,
            Face::Missing => crate::shared::Face::Missing,
            Face::Owned(id) => crate::shared::Face::Owned(
                self.fonts
                    .get(&id)
                    .cloned()
                    .ok_or_else(|| Fault("unknown font resource".into()))?,
            ),
        };
        let font = crate::shared::Font {
            face,
            size: font.size,
            weight: font.weight,
        };
        font.validate()?;
        Ok(font)
    }
    pub fn image(&self, id: u64) -> Result<Raster, Fault> {
        self.rasters
            .get(&id)
            .cloned()
            .ok_or_else(|| Fault("unknown raster resource".into()))
    }
}
