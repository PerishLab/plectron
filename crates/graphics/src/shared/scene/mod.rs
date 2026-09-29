mod area;
pub mod wire;
pub use area::Area;
mod pass;
mod plan;
pub use pass::Pass;
mod record;
pub use record::{Ink, Record};
mod space;
mod validate;
mod view;
use crate::shared::{Extent, Fault, Path, Rect, Scalar, Transform};
pub use font::{Bitmap, Face, Font, Weight};
pub use plan::{Layer, Primitive};
use serde::{Deserialize, Serialize};
pub use space::revision::{CHECKS, Effect, Effects, Routing};
pub use space::{Mapping, Space, Spatial};
use std::collections::BTreeMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
pub use textengine::font;
pub use view::{Frame, Impact, Materialization, Visual};

static SCENE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Glyph {
    pub x: Scalar,
    pub y: Scalar,
    pub id: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Item {
    Draw {
        origin: [usize; 2],
        clip: Option<Area>,
        record: Arc<Record>,
    },
    Group {
        space: Space,
        opacity: u8,
        items: Arc<[Item]>,
    },
    Fill {
        rect: Rect,
        color: Option<u32>,
    },
    Text {
        font: Font,
        clip: Option<Area>,
        glyphs: Vec<Glyph>,
        color: u32,
    },
}

#[derive(Debug, Clone)]
pub struct Scene {
    id: u64,
    generation: u64,
    extent: Extent,
    revision: u64,
    items: Arc<[Item]>,
    spaces: Arc<space::Index>,
    effects: Arc<BTreeMap<u64, u8>>,
    routing: Routing,
}

impl Scene {
    pub fn new(extent: Extent, revision: u64, items: Vec<Item>) -> Result<Self, Fault> {
        use pipeline::Graph;
        pipeline::step(crate::shared::operation::Publication).run(
            crate::shared::operation::Source {
                extent,
                revision,
                items,
            },
        )
    }
    pub fn project(extent: Extent, revision: u64, nodes: Vec<Spatial>) -> Result<Self, Fault> {
        limits(extent)?;
        let spaces = Arc::new(space::Index::project(&nodes)?);
        let id = identifier()?;
        Ok(Self {
            id,
            generation: id,
            extent,
            revision,
            items: Arc::default(),
            spaces,
            effects: Arc::default(),
            routing: Routing::default(),
        })
    }
    pub(crate) fn publish(extent: Extent, revision: u64, items: Vec<Item>) -> Result<Self, Fault> {
        limits(extent)?;
        let spaces = Arc::new(validate::check(&items)?);
        let id = identifier()?;
        Ok(Self {
            id,
            generation: id,
            extent,
            revision,
            items: items.into(),
            spaces,
            effects: Arc::default(),
            routing: Routing::default(),
        })
    }

    pub fn id(&self) -> u64 {
        self.id
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn items(&self) -> &[Item] {
        &self.items
    }
    pub fn range(&self, space: u64, viewport: Extent) -> Option<usize> {
        if space == 0 {
            Some(self.extent.height.saturating_sub(viewport.height))
        } else {
            self.spaces.range(space)
        }
    }
}

fn limits(extent: Extent) -> Result<(), Fault> {
    if [extent.width, extent.height]
        .iter()
        .all(|dimension| (1..=1_048_576).contains(dimension))
    {
        Ok(())
    } else {
        Err(Fault("scene coordinate budget exhausted".into()))
    }
}

enum Paint {
    Image(Area, crate::shared::Raster, Option<Area>),
    Fill(Area, u32),
    Path(Path, Transform, Option<Area>, u32),
}

impl Item {
    fn paint(
        &self,
        output: &mut Vec<Paint>,
        lookup: &mut impl FnMut(&Font, u16) -> Result<Path, Fault>,
    ) -> Result<(), Fault> {
        match self {
            Item::Draw {
                origin,
                clip,
                record,
            } => record.paint(*origin, *clip, output, lookup)?,
            Item::Fill {
                rect,
                color: Some(color),
            } => output.push(Paint::Fill(Area::from(*rect), *color)),
            Item::Text {
                font,
                clip,
                glyphs,
                color,
            } => {
                for glyph in glyphs {
                    glyph.paint((font, *color), ([0; 2], *clip), output, lookup)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
}

impl Glyph {
    fn paint(
        &self,
        brush: (&Font, u32),
        placement: ([usize; 2], Option<Area>),
        output: &mut Vec<Paint>,
        lookup: &mut impl FnMut(&Font, u16) -> Result<Path, Fault>,
    ) -> Result<(), Fault> {
        let (font, color) = brush;
        let (origin, clip) = placement;
        if font.source().is_some() {
            output.push(Paint::Path(
                lookup(font, self.id)?,
                font.transform().translate([
                    self.x + (origin[0] as i64).into(),
                    self.y + (origin[1] as i64).into(),
                ]),
                clip,
                color,
            ));
            return Ok(());
        }
        let bitmap = font.glyph(self.id)?;
        for (row, bits) in bitmap.rows.iter().enumerate() {
            for column in 0..bitmap.width {
                if bits & (1 << column) != 0 {
                    output.push(Paint::Fill(
                        Area {
                            x: i128::from(self.x.floor())
                                + origin[0] as i128
                                + font.scale(column) as i128,
                            y: i128::from(self.y.floor())
                                + origin[1] as i128
                                + font.scale(row) as i128,
                            width: font.scale(column + 1) - font.scale(column),
                            height: font.scale(row + 1) - font.scale(row),
                        }
                        .clip(clip),
                        color,
                    ));
                }
            }
        }
        Ok(())
    }
}

fn identifier() -> Result<u64, Fault> {
    SCENE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| Fault("render identifiers exhausted".into()))
}
