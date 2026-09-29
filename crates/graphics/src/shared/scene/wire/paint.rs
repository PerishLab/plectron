use super::{Font, Resources};
use crate::shared::{Fault, Glyph, Path, Rect, Space, Transform};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Ink {
    Image {
        rect: Rect,
        raster: u64,
    },
    Path {
        path: Path,
        transform: Transform,
        color: u32,
    },
    Fill {
        rect: Rect,
        color: Option<u32>,
    },
    Text {
        font: Font,
        glyphs: Vec<Glyph>,
        color: u32,
    },
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Item {
    Draw {
        origin: [usize; 2],
        clip: Option<crate::shared::Area>,
        record: Ink,
    },
    Group {
        space: Space,
        opacity: u8,
        items: Vec<Item>,
    },
    Fill {
        rect: Rect,
        color: Option<u32>,
    },
    Text {
        font: Font,
        clip: Option<crate::shared::Area>,
        glyphs: Vec<Glyph>,
        color: u32,
    },
}
impl Resources {
    pub fn ink(&mut self, ink: crate::shared::Ink) -> Ink {
        match ink {
            crate::shared::Ink::Image { rect, raster } => Ink::Image {
                rect,
                raster: self.raster(raster),
            },
            crate::shared::Ink::Text {
                font,
                glyphs,
                color,
            } => Ink::Text {
                font: self.font(font),
                glyphs: glyphs.to_vec(),
                color,
            },
            crate::shared::Ink::Path {
                path,
                transform,
                color,
            } => Ink::Path {
                path,
                transform,
                color,
            },
            crate::shared::Ink::Fill { rect, color } => Ink::Fill { rect, color },
        }
    }
    pub fn item(&mut self, item: crate::shared::Item) -> Item {
        match item {
            crate::shared::Item::Draw {
                origin,
                clip,
                record,
            } => Item::Draw {
                origin,
                clip,
                record: self.ink(record.ink().clone()),
            },
            crate::shared::Item::Group {
                space,
                opacity,
                items,
            } => Item::Group {
                space,
                opacity,
                items: items.iter().cloned().map(|item| self.item(item)).collect(),
            },
            crate::shared::Item::Fill { rect, color } => Item::Fill { rect, color },
            crate::shared::Item::Text {
                font,
                clip,
                glyphs,
                color,
            } => Item::Text {
                font: self.font(font),
                clip,
                glyphs,
                color,
            },
        }
    }
    pub fn record(&self, ink: Ink) -> Result<crate::shared::Record, Fault> {
        let ink = match ink {
            Ink::Image { rect, raster } => crate::shared::Ink::Image {
                rect,
                raster: self.image(raster)?,
            },
            Ink::Text {
                font,
                glyphs,
                color,
            } => crate::shared::Ink::Text {
                font: self.resolve(font)?,
                glyphs: glyphs.into(),
                color,
            },
            Ink::Path {
                path,
                transform,
                color,
            } => crate::shared::Ink::Path {
                path,
                transform,
                color,
            },
            Ink::Fill { rect, color } => crate::shared::Ink::Fill { rect, color },
        };
        crate::shared::Record::new(Arc::new(ink))
    }
    pub fn restore(&self, item: Item) -> Result<crate::shared::Item, Fault> {
        Ok(match item {
            Item::Draw {
                origin,
                clip,
                record,
            } => crate::shared::Item::Draw {
                origin,
                clip,
                record: Arc::new(self.record(record)?),
            },
            Item::Group {
                space,
                opacity,
                items,
            } => crate::shared::Item::Group {
                space,
                opacity,
                items: items
                    .into_iter()
                    .map(|item| self.restore(item))
                    .collect::<Result<_, _>>()?,
            },
            Item::Fill { rect, color } => crate::shared::Item::Fill { rect, color },
            Item::Text {
                font,
                clip,
                glyphs,
                color,
            } => crate::shared::Item::Text {
                font: self.resolve(font)?,
                clip,
                glyphs,
                color,
            },
        })
    }
}
