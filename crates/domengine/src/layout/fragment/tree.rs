use super::{Local, Placement};
use crate::layout::{
    measure::{Basis, History, Material, Source},
    text::paragraph::Lines,
};
use std::sync::Arc;

#[derive(Clone)]
pub(in crate::layout) struct Identity {
    owner: History,
    source: Source,
}
impl Identity {
    pub fn transition(&self, branch: &Branch) -> Option<crate::layout::delta::Transition> {
        let delta = branch.owner.transition(&self.owner)?;
        if self.source.matches(&branch.basis) || self.source.equivalent(&branch.basis)? {
            Some(delta)
        } else {
            None
        }
    }
    pub fn matches(&self, branch: &Branch) -> bool {
        branch.owner.owned(&self.owner) && self.source.matches(&branch.basis)
    }
}

pub struct Branch {
    pub(in crate::layout) token: Arc<()>,
    pub(in crate::layout) subtree: Arc<()>,
    pub(in crate::layout) owner: Material,
    pub(in crate::layout) basis: Basis,
    pub(in crate::layout) bindings: Vec<(crate::Node, Arc<Lines>)>,
    pub(in crate::layout) fragments: Vec<Placement<Local>>,
    pub(in crate::layout) links: Vec<Link>,
    pub(in crate::layout) count: usize,
    pub(in crate::layout) groups: usize,
    pub(in crate::layout) bounds: [usize; 2],
    pub(in crate::layout) targets: Option<Vec<Target>>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::layout) struct Target {
    pub space: u64,
    pub bounds: crate::Rect,
}
impl std::fmt::Debug for Branch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Branch")
            .field("count", &self.count)
            .field("links", &self.links.len())
            .finish()
    }
}
#[derive(Debug, Clone)]
pub struct Link {
    pub at: usize,
    pub parent: Option<usize>,
    pub offset: [usize; 2],
    pub branch: Arc<Branch>,
}
#[derive(Debug, Clone)]
pub struct Piece {
    pub(super) branch: Arc<Branch>,
    pub(super) index: usize,
}
impl std::ops::Deref for Piece {
    type Target = Local;
    fn deref(&self) -> &Local {
        &self.branch.fragments[self.index].local
    }
}
impl std::borrow::Borrow<Local> for Piece {
    fn borrow(&self) -> &Local {
        self
    }
}
impl Piece {
    pub fn same(&self, other: &Self) -> bool {
        self.index == other.index && Arc::ptr_eq(&self.branch, &other.branch)
    }
}
impl Branch {
    pub(in crate::layout) fn id(&self) -> crate::layout::Id {
        self.basis.id
    }
    pub(in crate::layout) fn same(&self, other: &Self) -> bool {
        (
            &self.fragments,
            self.count,
            self.groups,
            self.bounds,
            &self.targets,
        ) == (
            &other.fragments,
            other.count,
            other.groups,
            other.bounds,
            &other.targets,
        )
    }
    pub(in crate::layout) fn identity(&self) -> Identity {
        Identity {
            owner: self.owner.history(),
            source: self.basis.history(),
        }
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn fragments(&self) -> &[Placement<Local>] {
        &self.fragments
    }
    pub fn links(&self) -> &[Link] {
        &self.links
    }
}

impl Branch {
    pub fn visit(&self, visit: &mut impl FnMut(&Local)) {
        let mut links = self.links.iter().peekable();
        for index in 0..=self.fragments.len() {
            while links.peek().is_some_and(|link| link.at == index) {
                links.next().unwrap().branch.visit(visit);
            }
            if let Some(fragment) = self.fragments.get(index) {
                visit(&fragment.local);
            }
        }
    }
}
