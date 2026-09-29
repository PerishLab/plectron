use super::tree::Piece;
use super::{Content, Fragment, Local};
use crate::{Layout, Rect};
use graphics::shared::{Frame, Mapping};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement<L = Piece> {
    pub id: usize,
    pub parent: Option<usize>,
    pub offset: [usize; 2],
    pub local: L,
    pub(in crate::layout) origin: [usize; 2],
    pub(in crate::layout) clip: Rect,
}

impl Placement<Local> {
    pub(in crate::layout) fn new(fragment: Fragment, parent: [usize; 2]) -> Self {
        let origin = match fragment.content {
            Content::Box { .. } | Content::Inline { .. } => fragment.origin(),
            _ => parent,
        };
        Self {
            id: fragment.id,
            parent: fragment.parent,
            offset: std::array::from_fn(|axis| origin[axis] - parent[axis]),
            clip: fragment.clip,
            origin,
            local: Local::new(fragment, origin),
        }
    }
}
impl<L: std::borrow::Borrow<Local>> Placement<L> {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.origin[0] + self.local.borrow().rect.x,
            y: self.origin[1] + self.local.borrow().rect.y,
            ..self.local.borrow().rect
        }
    }
    pub fn resolve(&self) -> Fragment {
        Fragment {
            id: self.id,
            parent: self.parent,
            space: self.local.borrow().space,
            node: self.local.borrow().node,
            rect: self.rect(),
            clip: self.clip,
            content: self
                .local
                .borrow()
                .content
                .clone()
                .geometry(|area| self.area(area)),
            target: self.local.borrow().target,
        }
    }
    pub(in crate::layout) fn area(&self, area: graphics::shared::Area) -> Rect {
        area.map(self.origin.map(|value| value as i64))
    }
    pub(in crate::layout) fn context(&self) -> Rect {
        match self.local.borrow().content {
            Content::Box {
                viewport: Some(viewport),
                ..
            } => self.clip.intersect(self.area(viewport)),
            _ => self.clip,
        }
    }
}

impl Layout {
    pub fn placements(&self) -> &[Placement] {
        self.placements.get_or_init(|| self.root.expand())
    }
    pub fn mapping(&self, frame: &Frame, fragment: &Fragment) -> Option<Mapping> {
        let mut mapping = frame.mapping(fragment.space)?.clone();
        let origin = self.placements().get(fragment.id)?.origin;
        for (translation, origin) in mapping.translation.iter_mut().zip(origin) {
            *translation -= origin as i64;
        }
        Some(mapping)
    }
}
