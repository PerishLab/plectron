use super::{Content, Fragment};
use crate::{Node, Rect};
use graphics::shared::{Area, Font, Raster};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Local {
    pub space: u64,
    pub node: Option<usize>,
    pub rect: Rect,
    pub content: Content<Font, Raster, Area>,
    pub(in crate::layout) target: Option<Node>,
}

impl Local {
    pub(super) fn new(fragment: Fragment, origin: [usize; 2]) -> Self {
        Self {
            space: fragment.space,
            node: fragment.node,
            rect: Rect {
                x: fragment.rect.x - origin[0],
                y: fragment.rect.y - origin[1],
                ..fragment.rect
            },
            content: fragment
                .content
                .geometry(|rect| Area::relative(rect, origin)),
            target: fragment.target,
        }
    }
}
