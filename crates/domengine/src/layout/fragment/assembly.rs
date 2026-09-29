use super::{
    builder::Builder,
    tree::{Branch, Link, Target},
};
use crate::layout::{constraints::LIMIT, measure::View};
use crate::{Content, Fault, Rect};
use std::sync::Arc;

impl Builder<'_> {
    pub(in crate::layout) fn emit(
        &mut self,
        measured: &View<'_>,
        origin: [usize; 2],
        context: (Option<usize>, Rect),
    ) -> Result<(), Fault> {
        if measured.text.is_some() {
            return self.project(measured, origin, context);
        }
        let offset = std::array::from_fn(|axis| origin[axis] + measured.offset[axis]);
        let prior = self.previous.as_ref().and_then(|branch| {
            branch
                .links
                .iter()
                .find(|link| link.branch.id() == measured.id)
                .map(|link| link.branch.clone())
        });
        let branch = self
            .projection
            .build(measured, (Some(self.owner), offset), prior.as_ref())?;
        self.links.push(Link {
            at: self.placements.len(),
            parent: context.0,
            offset,
            branch,
        });
        Ok(())
    }
    pub(in crate::layout) fn finish(self, measured: &View<'_>) -> Result<Arc<Branch>, Fault> {
        let count =
            self.placements.len() + self.links.iter().map(|l| l.branch.count).sum::<usize>();
        let mut bounds = [0; 2];
        let mut groups = 0;
        let mut targets = Some(Vec::new());
        for p in &self.placements {
            let rect = p.rect();
            bounds[0] = bounds[0].max(rect.x + rect.width);
            bounds[1] = bounds[1].max(rect.y + rect.height);
            groups += match p.local.content {
                Content::Box { .. } => 2,
                Content::Inline { .. } => 1,
                _ => 0,
            };
            if p.local.target.is_some() {
                include(
                    &mut targets,
                    Target {
                        space: p.local.space,
                        bounds: p.local.rect,
                    },
                    self.query,
                );
            }
        }
        for link in &self.links {
            groups += link.branch.groups;
            if let Some(nested) = &link.branch.targets {
                for target in nested.iter() {
                    include(&mut targets, *target, self.query);
                }
            } else {
                targets = None;
            }
            for (axis, bound) in bounds.iter_mut().enumerate() {
                *bound = (*bound).max(link.offset[axis] + link.branch.bounds[axis]);
            }
        }
        if count > 65_536 || bounds.into_iter().any(|b| b > LIMIT) {
            return Err(Fault("fragment budget exhausted".into()));
        }
        let previous = self.previous.clone();
        let mut branch = Branch {
            token: Arc::new(()),
            subtree: Arc::new(()),
            owner: measured.material().clone(),
            basis: measured.basis(),
            bindings: measured
                .children()
                .filter(|child| child.material().inline().is_none())
                .filter_map(|child| {
                    child
                        .text
                        .map(|lines| (child.material().node(), lines.clone()))
                })
                .collect(),
            fragments: self.placements,
            links: self.links,
            count,
            groups,
            bounds,
            targets,
        };
        if let Some(previous) = previous.filter(|old| branch.same(old)) {
            branch.token = previous.token.clone();
            if branch.links.len() == previous.links.len()
                && branch.links.iter().zip(&previous.links).all(|(a, b)| {
                    (a.at, a.parent, a.offset) == (b.at, b.parent, b.offset)
                        && Arc::ptr_eq(&a.branch.subtree, &b.branch.subtree)
                })
            {
                branch.subtree = previous.subtree.clone();
            }
        }
        Ok(Arc::new(branch))
    }
}
fn include(targets: &mut Option<Vec<Target>>, target: Target, limit: usize) {
    let Some(entries) = targets.as_mut() else {
        return;
    };
    if let Some(existing) = entries.iter_mut().find(|value| value.space == target.space) {
        existing.bounds = existing.bounds.union(target.bounds);
    } else if entries.len() < limit {
        entries.push(target);
    } else {
        *targets = None;
    }
}
