use super::{Boundary, Cause, Change, Journal, Scroll, fact};
use crate::Fault;
use domengine::{Layout, Node};
use graphics::shared::{Scene, Visual};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct State {
    pub node: Option<Node>,
    pub offset: usize,
    pub range: usize,
    pub edge: Boundary,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Source {
    scene: u64,
    revision: u64,
    layout: u64,
    extent: domengine::Extent,
}

pub(in crate::display) struct Context<'a> {
    pub reasons: &'a BTreeMap<u64, Cause>,
    pub scene: Option<&'a Scene>,
    pub layout: Option<&'a Layout>,
    pub replay: &'a [Node],
}

impl Journal {
    pub(super) fn project(
        &mut self,
        visual: &Visual,
        context: Context<'_>,
        watching: &BTreeSet<Node>,
    ) -> Result<(), Fault> {
        let source = context
            .scene
            .zip(context.layout)
            .map(|(scene, layout)| Source {
                scene: scene.id(),
                revision: scene.revision(),
                layout: layout.revision(),
                extent: visual.extent,
            });
        if self.repeated(source, visual, context.replay) {
            return Ok(());
        }
        let started = std::time::Instant::now();
        self.work.updates = self.work.updates.saturating_add(1);
        let reusable = source.is_some() && source == self.source && context.replay.is_empty();
        let mut next = self.states(visual, context.scene, context.layout, reusable);
        self.work.spaces = self.work.spaces.saturating_add(next.len() as u64);
        let spaces = self
            .states
            .keys()
            .chain(next.keys())
            .copied()
            .collect::<BTreeSet<_>>();
        self.revision(&spaces, &next, context.replay)?;
        for space in spaces {
            self.change(space, &next, &context, watching);
        }
        self.states = std::mem::take(&mut next);
        self.source = source;
        self.work.elapsed = self
            .work
            .elapsed
            .saturating_add(crate::page::elapsed(started));
        Ok(())
    }

    fn repeated(&self, source: Option<Source>, visual: &Visual, replay: &[Node]) -> bool {
        let same = source.is_some() && source == self.source;
        let settled = replay.is_empty()
            && self
                .states
                .iter()
                .all(|(space, state)| state.offset == visual.offset(*space));
        same && settled
    }

    fn states(
        &mut self,
        visual: &Visual,
        scene: Option<&Scene>,
        layout: Option<&Layout>,
        reusable: bool,
    ) -> BTreeMap<u64, State> {
        if reusable {
            self.work.incremental = self.work.incremental.saturating_add(1);
            return self
                .states
                .iter()
                .map(|(space, state)| {
                    let offset = visual.offset(*space);
                    (
                        *space,
                        State {
                            offset,
                            edge: boundary(offset, state.range),
                            ..*state
                        },
                    )
                })
                .collect();
        }
        self.work.rebuilds = self.work.rebuilds.saturating_add(1);
        rebuild(visual, scene, layout)
    }

    fn revision(
        &mut self,
        spaces: &BTreeSet<u64>,
        next: &BTreeMap<u64, State>,
        replay: &[Node],
    ) -> Result<(), Fault> {
        let changed = spaces.iter().any(|space| {
            self.states.get(space) != next.get(space)
                || next
                    .get(space)
                    .and_then(|state| state.node)
                    .is_some_and(|node| replay.contains(&node))
        });
        if changed {
            self.revision = self
                .revision
                .checked_add(1)
                .ok_or_else(|| Fault("scroll revisions exhausted".into()))?;
        }
        Ok(())
    }

    fn change(
        &mut self,
        space: u64,
        next: &BTreeMap<u64, State>,
        context: &Context<'_>,
        watching: &BTreeSet<Node>,
    ) {
        let previous = self.states.get(&space).copied();
        let current = next.get(&space).copied();
        let replayed = current
            .and_then(|state| state.node)
            .is_some_and(|node| context.replay.contains(&node));
        if previous == current && !replayed {
            return;
        }
        let state = current.or(previous).expect("projected scroll state");
        let cause = context
            .reasons
            .get(&space)
            .copied()
            .unwrap_or(Cause::Layout);
        let position = current.and_then(|state| {
            let node = state.node?;
            let layout = context.layout?;
            self.work.positions = self.work.positions.saturating_add(1);
            super::super::control::Geometry::new(layout).position(super::super::control::Anchor {
                node,
                offset: state.offset,
            })
        });
        self.record(Change {
            node: state.node,
            position,
            notify: state
                .node
                .is_some_and(|node| watching.contains(&node) || context.replay.contains(&node)),
            fact: Scroll {
                target: state.node.map(Node::id),
                space,
                previous: previous.map_or(state.offset, |state| state.offset),
                offset: current.map_or(0, |state| state.offset),
                range: current.map_or(0, |state| state.range),
                edge: current.map_or(Boundary::Start, |state| state.edge),
                cause,
                causes: vec![cause],
                revision: self.revision,
                position: position.map(fact),
            },
        });
    }
}

fn rebuild(
    visual: &Visual,
    scene: Option<&Scene>,
    layout: Option<&Layout>,
) -> BTreeMap<u64, State> {
    let mut next = BTreeMap::new();
    let Some(scene) = scene else {
        return next;
    };
    let mut nodes = layout
        .map(|layout| super::super::control::Geometry::new(layout).scrollnodes())
        .unwrap_or_default();
    let range = scene.range(0, visual.extent).unwrap_or_default();
    let offset = visual.offset(0);
    next.insert(
        0,
        State {
            node: nodes.remove(&0),
            offset,
            range,
            edge: boundary(offset, range),
        },
    );
    for (space, node) in nodes {
        let Some(range) = scene.range(space, visual.extent) else {
            continue;
        };
        let offset = visual.offset(space);
        next.insert(
            space,
            State {
                node: Some(node),
                offset,
                range,
                edge: boundary(offset, range),
            },
        );
    }
    next
}

fn boundary(offset: usize, range: usize) -> Boundary {
    if offset == 0 {
        Boundary::Start
    } else if offset == range {
        Boundary::End
    } else {
        Boundary::Middle
    }
}
