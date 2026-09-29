use super::{Area, Font, Glyph, Paint};
use crate::shared::{Fault, Path, Rect, Transform};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Ink {
    Image {
        rect: Rect,
        raster: crate::shared::Raster,
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
        glyphs: Arc<[Glyph]>,
        color: u32,
    },
}

#[derive(Debug, Clone, Serialize)]
#[serde(into = "Ink")]
pub struct Record {
    ink: Arc<Ink>,
    cost: usize,
}
impl Record {
    pub fn new(ink: Arc<Ink>) -> Result<Self, Fault> {
        let (color, cost) = match ink.as_ref() {
            Ink::Image { rect, .. } => {
                if rect.x.saturating_add(rect.width) > 1_048_576
                    || rect.y.saturating_add(rect.height) > 1_048_576
                {
                    return Err(Fault("image geometry budget exhausted".into()));
                }
                (0, 1)
            }
            Ink::Path {
                path,
                transform,
                color,
            } => {
                if transform
                    .0
                    .iter()
                    .any(|value| value.value().abs() > 1_048_576.0)
                {
                    return Err(Fault("path transform budget exhausted".into()));
                }
                (*color, 1 + path.segments().len())
            }
            Ink::Fill { color, .. } => (color.unwrap_or(0), 1),
            Ink::Text {
                font,
                glyphs,
                color,
            } => {
                font.validate()?;
                if glyphs.len() >= 65_536 {
                    return Err(Fault("scene budget exhausted".into()));
                }
                for glyph in glyphs.iter() {
                    font.advance(glyph.id)?;
                }
                (*color, 1 + glyphs.len())
            }
        };
        super::validate::colorcheck(color)?;
        Ok(Self { ink, cost })
    }
    pub fn ink(&self) -> &Ink {
        &self.ink
    }
    pub fn cost(&self) -> usize {
        self.cost
    }
    pub(super) fn paint(
        &self,
        origin: [usize; 2],
        clip: Option<super::Area>,
        output: &mut Vec<Paint>,
        lookup: &mut impl FnMut(&Font, u16) -> Result<Path, Fault>,
    ) -> Result<(), Fault> {
        match self.ink.as_ref() {
            Ink::Image { rect, raster } => output.push(Paint::Image(
                Area {
                    x: rect.x as i128 + origin[0] as i128,
                    y: rect.y as i128 + origin[1] as i128,
                    width: rect.width,
                    height: rect.height,
                },
                raster.clone(),
                clip,
            )),
            Ink::Path {
                path,
                transform,
                color,
            } => output.push(Paint::Path(
                path.clone(),
                transform.translate(origin.map(|value| (value as i64).into())),
                clip,
                *color,
            )),
            Ink::Fill {
                rect,
                color: Some(color),
            } => output.push(Paint::Fill(
                Area {
                    x: rect.x as i128 + origin[0] as i128,
                    y: rect.y as i128 + origin[1] as i128,
                    width: rect.width,
                    height: rect.height,
                }
                .clip(clip),
                *color,
            )),
            Ink::Text {
                font,
                glyphs,
                color,
            } => {
                for glyph in glyphs.iter() {
                    glyph.paint((font, *color), (origin, clip), output, lookup)?;
                }
            }
            Ink::Fill { color: None, .. } => (),
        }
        Ok(())
    }
}
impl Ink {
    pub fn cost(&self) -> shared::cache::Cost {
        let mut cost = shared::cache::Cost::new(std::mem::size_of::<Self>());
        cost.bytes += match self {
            Self::Text { glyphs, font, .. } => {
                if let Some(resource) = font.resource() {
                    cost.resource(resource);
                }
                glyphs.len() * std::mem::size_of::<Glyph>()
            }
            Self::Fill { .. } => 0,
            Self::Image { raster, .. } => raster.retained(),
            Self::Path { path, .. } => path.retained(),
        };
        cost
    }
    pub fn weight(&self) -> usize {
        self.cost().total()
    }
}
impl<'de> Deserialize<'de> for Record {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let ink = Ink::deserialize(deserializer)?;
        Self::new(Arc::new(ink)).map_err(serde::de::Error::custom)
    }
}
impl From<Record> for Ink {
    fn from(record: Record) -> Self {
        (*record.ink).clone()
    }
}
