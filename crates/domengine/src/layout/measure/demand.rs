use crate::{Branch, Layout, Node, Rect};
use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub struct Boundary {
    pub node: Node,
    pub space: u64,
    pub size: [usize; 2],
    pub pending: bool,
    pub quality: geoengine::Quality,
    content: [usize; 2],
    fingerprint: u64,
    estimated: bool,
}
impl Boundary {
    pub(in crate::layout) fn content(&self) -> [usize; 2] {
        self.content
    }
    pub(in crate::layout) fn fingerprint(&self) -> u64 {
        self.fingerprint
    }
    pub(in crate::layout) fn estimated(&self) -> bool {
        self.estimated
    }
    pub fn visible(&self, frame: &graphics::shared::Frame, overscan: usize) -> bool {
        frame
            .prefetch(
                self.space,
                Rect {
                    x: 0,
                    y: 0,
                    width: self.size[0],
                    height: self.size[1],
                },
                overscan,
            )
            .is_some_and(|rect| rect.width > 0 && rect.height > 0)
    }
}
impl Layout {
    pub fn boundaries(&self) -> Vec<Boundary> {
        let mut result = Vec::new();
        self.root.boundaries(&mut result);
        result
    }
    pub fn demand(&self, frame: &graphics::shared::Frame, overscan: usize) -> BTreeSet<Node> {
        self.boundaries()
            .into_iter()
            .filter(|b| b.visible(frame, overscan))
            .map(|b| b.node)
            .collect()
    }
    pub fn ready(&self, frame: &graphics::shared::Frame) -> bool {
        self.boundaries()
            .iter()
            .all(|b| !b.pending || !b.visible(frame, 0))
    }
    pub fn geometry(&self, node: Node) -> Option<Rect> {
        let mut result: Option<Rect> = None;
        self.visit(|p| {
            if p.local.node == Some(node.id()) {
                result = Some(result.map_or(p.rect(), |old| old.union(p.rect())));
            }
        });
        result
    }
}
impl Branch {
    fn boundaries(&self, result: &mut Vec<Boundary>) {
        if !self.owner.isolated() {
            return;
        }
        if self.owner.independent() {
            result.push(Boundary {
                node: self.owner.node(),
                space: self.fragments[0].local.space,
                size: [
                    self.fragments[0].local.rect.width,
                    self.fragments[0].local.rect.height,
                ],
                pending: self.basis.pending,
                quality: self.basis.quality(),
                content: self.basis.size(),
                fingerprint: self.owner.fingerprint(),
                estimated: self.owner.estimated(),
            });
        }
        for link in &self.links {
            link.branch.boundaries(result);
        }
    }
}
