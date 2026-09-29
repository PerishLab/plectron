use crate::{Display, Fault};
use domengine::Node;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
mod kind;
mod project;
pub use kind::{Boundary, Cause, Position, Work};
pub(in crate::display) use project::Context;
use project::{Source, State};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Scroll {
    pub target: Option<usize>,
    pub space: u64,
    pub previous: usize,
    pub offset: usize,
    pub range: usize,
    pub edge: Boundary,
    pub cause: Cause,
    pub causes: Vec<Cause>,
    pub revision: u64,
    pub position: Option<Position>,
}

#[derive(Debug, Clone)]
pub(crate) struct Change {
    pub node: Option<Node>,
    pub fact: Scroll,
    pub position: Option<super::BookmarkPosition>,
    pub notify: bool,
}

#[derive(Default)]
pub(crate) struct Journal {
    states: BTreeMap<u64, State>,
    pending: BTreeMap<u64, Change>,
    order: Vec<u64>,
    history: VecDeque<Scroll>,
    revision: u64,
    source: Option<Source>,
    work: Work,
}

impl Journal {
    fn record(&mut self, change: Change) {
        if !self.pending.contains_key(&change.fact.space) {
            self.order.push(change.fact.space);
        }
        self.pending
            .entry(change.fact.space)
            .and_modify(|current| {
                current.node = change.node.or(current.node);
                current.fact.target = change.fact.target.or(current.fact.target);
                current.fact.offset = change.fact.offset;
                current.fact.range = change.fact.range;
                current.fact.edge = change.fact.edge;
                current.fact.cause = change.fact.cause;
                for cause in &change.fact.causes {
                    if !current.fact.causes.contains(cause) {
                        current.fact.causes.push(*cause);
                    }
                }
                current.fact.revision = change.fact.revision;
                current.fact.position = change.fact.position.clone();
                current.position = change.position;
                current.notify |= change.notify;
            })
            .or_insert(change);
    }

    pub fn pending(&self) -> Vec<Change> {
        self.order
            .iter()
            .filter_map(|space| self.pending.get(space).cloned())
            .collect()
    }

    pub fn drain(&mut self) -> Vec<Change> {
        let changes = std::mem::take(&mut self.order)
            .into_iter()
            .filter_map(|space| self.pending.remove(&space))
            .collect::<Vec<_>>();
        for change in &changes {
            self.history.push_back(change.fact.clone());
            if self.history.len() > 32 {
                self.history.pop_front();
            }
        }
        changes
    }

    pub fn snapshot(&self) -> Vec<Scroll> {
        self.history
            .iter()
            .cloned()
            .chain(
                self.order
                    .iter()
                    .filter_map(|space| self.pending.get(space).map(|change| change.fact.clone())),
            )
            .collect()
    }

    pub fn work(&self) -> Work {
        self.work
    }
}

fn fact(position: super::BookmarkPosition) -> Position {
    let point = |bookmark: super::Bookmark| Position {
        target: bookmark.target.id(),
        inset: bookmark.inset,
        fallbacks: Vec::new(),
    };
    let mut anchor = point(position.anchor);
    anchor.fallbacks = position
        .fallbacks
        .into_iter()
        .flatten()
        .map(point)
        .collect();
    anchor
}

impl Display {
    pub(crate) fn pending(&self) -> Vec<Change> {
        self.journal.pending()
    }

    pub(crate) fn take(&mut self) -> Vec<Change> {
        self.journal.drain()
    }

    pub fn scrolling(&self) -> Vec<Scroll> {
        self.journal.snapshot()
    }

    pub fn scrollwork(&self) -> Work {
        self.journal.work()
    }

    pub(super) fn offsets(
        &mut self,
        offsets: BTreeMap<u64, usize>,
        context: Context<'_>,
    ) -> Result<bool, Fault> {
        let changed = offsets != self.visual.offsets;
        if changed {
            self.visual.revision = self
                .visual
                .revision
                .checked_add(1)
                .ok_or_else(|| Fault("visual revisions exhausted".into()))?;
            self.visual.offsets = offsets;
        }
        self.journal
            .project(&self.visual, context, &self.watching)?;
        Ok(changed)
    }
}
