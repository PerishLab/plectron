use crate::shared::Fault;
mod raster;
pub use raster::Raster;
mod sample;
pub use sample::Sample;
mod output;
pub use output::Output;
mod clip;
pub use clip::Clip;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Extent {
    pub width: usize,
    pub height: usize,
}

impl Extent {
    pub fn area(self) -> Result<usize, Fault> {
        if self.width == 0 || self.height == 0 {
            return Err(Fault("extent must be positive".into()));
        }
        match self.width.checked_mul(self.height) {
            Some(area) if area <= 4_194_304 => Ok(area),
            _ => Err(Fault("pixel budget exhausted".into())),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub fn union(self, other: Self) -> Self {
        if self.width == 0 || self.height == 0 {
            return other;
        }
        if other.width == 0 || other.height == 0 {
            return self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        Self {
            x,
            y,
            width: self
                .x
                .saturating_add(self.width)
                .max(other.x.saturating_add(other.width))
                - x,
            height: self
                .y
                .saturating_add(self.height)
                .max(other.y.saturating_add(other.height))
                - y,
        }
    }

    pub fn contains(self, x: usize, y: usize) -> bool {
        (self.x..self.x.saturating_add(self.width)).contains(&x)
            && (self.y..self.y.saturating_add(self.height)).contains(&y)
    }

    pub fn intersect(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self {
            x,
            y,
            width: (self.x.saturating_add(self.width))
                .min(other.x.saturating_add(other.width))
                .saturating_sub(x),
            height: (self.y.saturating_add(self.height))
                .min(other.y.saturating_add(other.height))
                .saturating_sub(y),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub extent: Extent,
    pub pixels: Vec<u32>,
}
pub use shared::geometry::{Path, Point, Scalar, Segment, Transform};
pub mod coverage;
mod shape;
pub use coverage::Coverage;
pub use shape::{Evaluation, Geometry};
mod curve;
pub use curve::Curve;
mod bands;
mod prepare;
pub use bands::Bands;
