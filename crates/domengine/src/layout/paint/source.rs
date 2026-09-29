use crate::{Content, Local, Rect};
use graphics::shared::Ink;

pub(super) struct Source {
    pub origin: [usize; 2],
    pub ink: Ink,
}
impl Source {
    pub fn project(fragment: &Local) -> Self {
        let origin = [fragment.rect.x, fragment.rect.y];
        let rect = Rect {
            x: 0,
            y: 0,
            width: fragment.rect.width,
            height: fragment.rect.height,
        };
        let ink = match &fragment.content {
            Content::Image { raster } => Ink::Image {
                rect,
                raster: raster.clone(),
            },
            Content::Box { color, .. } => Ink::Fill {
                rect,
                color: *color,
            },
            Content::Text {
                font,
                glyphs,
                color,
                ..
            } => Ink::Text {
                font: font.clone(),
                color: *color,
                glyphs: glyphs.clone(),
            },
            Content::Caret { color } | Content::Selection { color } => Ink::Fill {
                rect,
                color: Some(*color),
            },
            Content::Line | Content::Inline { .. } => {
                unreachable!("groups are composed separately")
            }
        };
        Self { origin, ink }
    }
}
