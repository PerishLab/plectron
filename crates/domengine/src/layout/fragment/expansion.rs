use super::{
    Content, Placement,
    tree::{Branch, Piece},
};
use crate::Rect;
use std::sync::Arc;

impl Branch {
    pub(in crate::layout) fn expand(self: &Arc<Self>) -> Vec<Placement> {
        let mut output = Vec::with_capacity(self.count);
        self.append(([0; 2], None), &mut output);
        output
    }
    fn append(
        self: &Arc<Self>,
        (base, parent): ([usize; 2], Option<usize>),
        output: &mut Vec<Placement>,
    ) {
        let mut indices = Vec::with_capacity(self.fragments.len());
        let mut links = self.links.iter().peekable();
        for index in 0..=self.fragments.len() {
            while links.peek().is_some_and(|link| link.at == index) {
                let link = links.next().unwrap();
                let origin = std::array::from_fn(|axis| base[axis] + link.offset[axis]);
                link.branch
                    .append((origin, link.parent.map(|p| indices[p]).or(parent)), output);
            }
            let Some(fragment) = self.fragments.get(index) else {
                break;
            };
            let parent = fragment.parent.map(|p| indices[p]).or(parent);
            let origin = std::array::from_fn(|axis| base[axis] + fragment.origin[axis]);
            let basis = parent.map_or([0; 2], |p| output[p].origin);
            let clip = parent.map_or(
                Rect {
                    x: 0,
                    y: 0,
                    width: super::super::constraints::LIMIT,
                    height: super::super::constraints::LIMIT,
                },
                |p| output[p].context(),
            );
            let mut placement = Placement {
                id: output.len(),
                parent,
                origin,
                clip,
                offset: std::array::from_fn(|axis| origin[axis] - basis[axis]),
                local: Piece {
                    branch: self.clone(),
                    index,
                },
            };
            if matches!(placement.local.content, Content::Image { .. }) {
                placement.clip = clip.intersect(placement.rect());
            }
            indices.push(placement.id);
            output.push(placement);
        }
    }
}
