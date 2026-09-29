mod identity;
pub mod operation;
mod painter;
mod placement;
mod recording;
mod report;
mod source;
mod spatial;
mod tree;
use super::{Bridge, Content, Layout, Local};
use crate::Item;
use graphics::shared::{Space, Spatial};
pub use painter::Painter;
use recording::Recording;
pub use report::Painting;

impl Layout {
    pub fn paint(&self) -> Vec<Item> {
        self.bridge(Bridge::Paint);
        vec![
            self.root
                .compose(&mut Reference)
                .expect("reference recording is infallible"),
        ]
    }
    pub fn spatial(&self) -> Vec<Spatial> {
        self.bridge(Bridge::Space);
        spatial::compose(&self.root).expect("spatial projection is infallible")
    }
    pub fn pick(&self, frame: &graphics::shared::Frame, x: usize, y: usize) -> Option<crate::Node> {
        self.region(frame, x, y)
            .and_then(|fragment| fragment.target)
    }
    pub fn scroller(&self, frame: &graphics::shared::Frame, x: usize, y: usize) -> Option<u64> {
        let fragment = self.region(frame, x, y)?;
        if matches!(
            fragment.content,
            Content::Box {
                scroll: Some(_),
                ..
            }
        ) {
            let content = fragment.space + 1;
            if frame
                .mapping(content)
                .is_some_and(|mapping| mapping.clip.contains(x, y))
            {
                return frame.scroller(content);
            }
        }
        frame.scroller(fragment.space)
    }
    pub fn region(&self, frame: &graphics::shared::Frame, x: usize, y: usize) -> Option<&Local> {
        self.bridge(Bridge::Hit);
        let mut observation = observe::scope("dom.hit");
        let result = self
            .root
            .targets(frame, x, y)
            .then(|| {
                self.root.find(
                    self.scope(),
                    &mut |branch| branch.targets(frame, x, y),
                    &mut |local, _| {
                        frame
                            .mapping(local.space)
                            .is_some_and(|mapping| mapping.map(local.rect).contains(x, y))
                    },
                )
            })
            .flatten();
        observation.finish(true);
        result
    }
}
pub(super) fn group(
    (fragment, offset): (&Local, [i64; 2]),
    draw: Item,
    descendants: Vec<Item>,
) -> Item {
    let Content::Box {
        color: _,
        translation,
        opacity,
        viewport,
        scroll,
    } = &fragment.content
    else {
        unreachable!()
    };
    let content = Item::Group {
        space: Space {
            id: fragment.space + 1,
            translation: [0, 0],
            clip: *viewport,
            scroll: *scroll,
        },
        opacity: 255,
        items: descendants.into(),
    };
    Item::Group {
        space: Space {
            id: fragment.space,
            translation: std::array::from_fn(|axis| translation[axis] + offset[axis]),
            clip: None,
            scroll: None,
        },
        opacity: *opacity,
        items: vec![draw, content].into(),
    }
}

struct Reference;
impl Recording for Reference {
    fn draw(&mut self, fragment: &Local) -> Result<Item, crate::Fault> {
        Ok(reference(fragment))
    }
}
fn reference(fragment: &Local) -> Item {
    let rect = fragment.rect;
    match &fragment.content {
        Content::Image { raster } => Item::Draw {
            origin: [0; 2],
            clip: None,
            record: std::sync::Arc::new(
                graphics::shared::Record::new(std::sync::Arc::new(graphics::shared::Ink::Image {
                    rect,
                    raster: raster.clone(),
                }))
                .expect("validated image geometry"),
            ),
        },
        Content::Box { color, .. } => Item::Fill {
            rect,
            color: *color,
        },
        Content::Text {
            font,
            glyphs,
            color,
            clip,
            ..
        } => Item::Text {
            font: font.clone(),
            glyphs: glyphs
                .iter()
                .map(|glyph| graphics::shared::Glyph {
                    id: glyph.id,
                    x: glyph.x + graphics::shared::Scalar::from(rect.x as i64),
                    y: glyph.y + graphics::shared::Scalar::from(rect.y as i64),
                })
                .collect(),
            color: *color,
            clip: *clip,
        },
        Content::Caret { color } => Item::Fill {
            rect,
            color: Some(*color),
        },
        Content::Selection { color } => Item::Fill {
            rect,
            color: Some(*color),
        },
        Content::Line | Content::Inline { .. } => unreachable!("groups are composed separately"),
    }
}
