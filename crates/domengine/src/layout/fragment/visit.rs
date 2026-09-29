use super::{Content, Local, Placement, tree::Branch};
use crate::{Layout, Rect};

#[derive(Clone, Copy)]
struct Scope {
    base: [usize; 2],
    origin: [usize; 2],
    parent: Option<usize>,
    clip: Rect,
}
struct Visitor<F> {
    scopes: Vec<Scope>,
    next: usize,
    visit: F,
}
impl Layout {
    pub fn visit(&self, visit: impl FnMut(Placement<&Local>)) {
        let (base, clip) = self.scope();
        Visitor {
            scopes: Vec::new(),
            next: 0,
            visit,
        }
        .walk(
            &self.root,
            Scope {
                base,
                origin: base,
                parent: None,
                clip,
            },
        );
    }
}
impl<F: FnMut(Placement<&Local>)> Visitor<F> {
    fn walk(&mut self, branch: &Branch, scope: Scope) {
        let start = self.scopes.len();
        let mut links = branch.links.iter().peekable();
        for index in 0..=branch.fragments.len() {
            while links.peek().is_some_and(|link| link.at == index) {
                let link = links.next().unwrap();
                let mut child = link.parent.map_or(scope, |p| self.scopes[start + p]);
                child.base = std::array::from_fn(|axis| scope.base[axis] + link.offset[axis]);
                self.walk(&link.branch, child);
            }
            let Some(p) = branch.fragments.get(index) else {
                break;
            };
            let context = p.parent.map_or(scope, |p| self.scopes[start + p]);
            let origin = std::array::from_fn(|axis| scope.base[axis] + p.origin[axis]);
            let mut placed = Placement {
                id: self.next,
                parent: context.parent,
                origin,
                clip: context.clip,
                offset: std::array::from_fn(|axis| origin[axis] - context.origin[axis]),
                local: &p.local,
            };
            if matches!(p.local.content, Content::Image { .. }) {
                placed.clip = placed.clip.intersect(placed.rect());
            }
            let clip = placed.context();
            (self.visit)(placed);
            self.scopes.push(Scope {
                base: scope.base,
                origin,
                parent: Some(self.next),
                clip,
            });
            self.next += 1;
        }
        self.scopes.truncate(start);
    }
}
