mod assembly;
pub(in crate::layout) mod builder;
mod content;
mod expansion;
mod local;
mod placement;
mod query;
pub(in crate::layout) mod tree;
mod visit;
use crate::{Node, Rect};
use graphics::shared::{Font, Glyph};
pub use local::Local;
pub use placement::Placement;
pub use query::Coverage;
use serde::{Deserialize, Serialize};
use std::ops::Range;
use std::sync::Arc;
pub use tree::{Branch, Link, Piece};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Content<F = Font, R = graphics::shared::Raster, C = Rect> {
    Image {
        raster: R,
    },
    Box {
        color: Option<u32>,
        translation: [i64; 2],
        opacity: u8,
        viewport: Option<C>,
        scroll: Option<usize>,
    },
    Line,
    Inline {
        translation: [i64; 2],
    },
    Selection {
        color: u32,
    },
    Caret {
        color: u32,
    },
    Text {
        font: F,
        glyphs: Arc<[Glyph]>,
        clusters: Arc<[Range<usize>]>,
        clip: Option<C>,
        color: u32,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Fragment<F = Font, R = graphics::shared::Raster> {
    pub id: usize,
    pub space: u64,
    pub parent: Option<usize>,
    pub node: Option<usize>,
    pub rect: Rect,
    pub clip: Rect,
    pub content: Content<F, R>,
    #[serde(skip)]
    pub(super) target: Option<Node>,
}

impl<F, R> Fragment<F, R> {
    pub fn target(&self) -> Option<Node> {
        self.target
    }

    pub fn origin(&self) -> [usize; 2] {
        [self.rect.x, self.rect.y]
    }
    pub fn map<G, S>(
        self,
        font: impl FnOnce(F) -> G,
        raster: impl FnOnce(R) -> S,
    ) -> Fragment<G, S> {
        let Self {
            id,
            space,
            parent,
            node,
            rect,
            clip,
            content,
            target,
        } = self;
        let content = match content {
            Content::Image { raster: value } => Content::Image {
                raster: raster(value),
            },
            Content::Text {
                font: value,
                glyphs,
                clusters,
                clip,
                color,
            } => Content::Text {
                font: font(value),
                glyphs,
                clusters,
                clip,
                color,
            },
            Content::Box {
                color,
                translation,
                opacity,
                viewport,
                scroll,
            } => Content::Box {
                color,
                translation,
                opacity,
                viewport,
                scroll,
            },
            Content::Line => Content::Line,
            Content::Inline { translation } => Content::Inline { translation },
            Content::Selection { color } => Content::Selection { color },
            Content::Caret { color } => Content::Caret { color },
        };
        Fragment {
            id,
            space,
            parent,
            node,
            rect,
            clip,
            content,
            target,
        }
    }
}
