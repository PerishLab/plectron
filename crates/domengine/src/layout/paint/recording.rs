use super::Painter;
use crate::{Branch, Fault, Item, Local};
use shared::cache::{Cost, Policy};
use shared::route::{self, Route};
use std::sync::Arc;

pub(super) const POLICY: Policy = Policy {
    read: true,
    write: true,
    entries: 4096,
    bytes: 16_777_216,
};
pub(super) trait Recording: Sized {
    fn draw(&mut self, fragment: &Local) -> Result<Item, Fault>;
    fn place(&mut self, branch: &Arc<Branch>, offset: [i64; 2]) -> Result<Item, Fault> {
        let items = branch.compose(self)?;
        Ok(if offset == [0; 2] {
            items
        } else {
            super::placement::shift(items, offset)
        })
    }
}
pub(super) struct Entry {
    identity: crate::layout::fragment::tree::Identity,
    item: Item,
    offset: [i64; 2],
}
impl Painter {
    pub(super) fn compose(&mut self, layout: &crate::Layout) -> Result<Vec<Item>, Fault> {
        self.place(layout.root(), [0; 2]).map(|item| vec![item])
    }
}
impl Recording for Painter {
    fn draw(&mut self, fragment: &Local) -> Result<Item, Fault> {
        self.draw(fragment)
    }
    fn place(&mut self, branch: &Arc<Branch>, offset: [i64; 2]) -> Result<Item, Fault> {
        if !self.report.enabled {
            self.report.built += 1;
            return branch
                .compose(self)
                .map(|items| super::placement::shift(items, offset));
        }
        let previous = self
            .retained
            .lookup(&branch.owner.node())
            .and_then(|entry| {
                let selected = route::select(entry.identity.matches(branch), self.routing, || {
                    self.report.checked += 1;
                    entry.identity.transition(branch)
                });
                match selected {
                    Route::Reuse => Some((entry.item.clone(), entry.offset, false)),
                    Route::Update(change) => {
                        self.report.routed += 1;
                        Some((
                            super::placement::update(entry.item.clone(), change),
                            entry.offset,
                            true,
                        ))
                    }
                    Route::Build => None,
                }
            });
        if let Some((_, placement, _)) = &previous {
            self.report.reused += branch.count;
            if *placement != offset {
                self.report.relocated += branch.count;
            }
            if offset != [0; 2] {
                self.report.placed += 1;
            }
        }
        let (item, changed) = match previous {
            Some((item, placement, updated)) => (item, updated || placement != offset),
            None => {
                self.report.built += 1;
                (branch.compose(self)?, true)
            }
        };
        if changed {
            let cost = Cost::new(std::mem::size_of::<Entry>());
            let entry = Entry {
                identity: branch.identity(),
                item: item.clone(),
                offset,
            };
            if let Err((entry, cost)) = self.retained.replace(&branch.owner.node(), entry, cost) {
                self.retained.store(branch.owner.node(), entry, cost);
            }
        }
        Ok(if offset == [0; 2] {
            item
        } else {
            super::placement::shift(item, offset)
        })
    }
}
