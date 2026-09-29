use super::Area;
use crate::shared::{Fault, Rect};
use serde::{Deserialize, Serialize};
mod index;
pub(super) mod revision;
pub(crate) use index::Index;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Space {
    pub id: u64,
    pub translation: [i64; 2],
    pub clip: Option<Area>,
    pub scroll: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spatial {
    pub space: Space,
    pub opacity: u8,
    pub parent: usize,
    pub descendants: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Mapping {
    pub id: u64,
    pub parent: Option<u64>,
    pub translation: [i64; 2],
    pub clip: Rect,
    pub range: Option<usize>,
}

impl Mapping {
    pub fn project(&self, area: Area) -> Rect {
        area.map(self.translation).intersect(self.clip)
    }
    pub fn map(&self, rect: Rect) -> Rect {
        translate(rect, self.translation).intersect(self.clip)
    }
    pub fn prefetch(&self, rect: Rect, margin: usize) -> Rect {
        let right = self
            .clip
            .x
            .saturating_add(self.clip.width)
            .saturating_add(margin);
        let bottom = self
            .clip
            .y
            .saturating_add(self.clip.height)
            .saturating_add(margin);
        let clip = Rect {
            x: self.clip.x.saturating_sub(margin),
            y: self.clip.y.saturating_sub(margin),
            width: right.saturating_sub(self.clip.x.saturating_sub(margin)),
            height: bottom.saturating_sub(self.clip.y.saturating_sub(margin)),
        };
        translate(rect, self.translation).intersect(clip)
    }
    pub(super) fn child(&self, space: &Space, offset: usize) -> Result<Self, Fault> {
        let x = self.translation[0].checked_add(space.translation[0]);
        let y = self.translation[1].checked_add(space.translation[1]);
        let (Some(x), Some(y)) = (x, y) else {
            return Err(Fault("space translation overflow".into()));
        };
        let clip = space
            .clip
            .map_or(self.clip, |clip| clip.map([x, y]).intersect(self.clip));
        Ok(Self {
            id: space.id,
            parent: Some(self.id),
            translation: [x, y - offset as i64],
            clip,
            range: space.scroll,
        })
    }
}

fn translate(rect: Rect, delta: [i64; 2]) -> Rect {
    super::Area::from(rect).map(delta)
}
