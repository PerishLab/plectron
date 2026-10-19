use domengine::{Layout, Node};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Edge {
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Policy {
    pub lease: u64,
    pub version: u64,
    pub node: Node,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Follow(Edge),
    Restore(Bookmark),
}

pub(super) struct Geometry<'a> {
    layout: &'a Layout,
}

impl<'a> Geometry<'a> {
    pub const fn new(layout: &'a Layout) -> Self {
        Self { layout }
    }

    pub fn scroller(&self, node: Node) -> Option<u64> {
        self.layout.fragments().iter().find_map(|fragment| {
            (fragment.target() == Some(node)
                && matches!(
                    fragment.content,
                    domengine::Content::Box {
                        scroll: Some(_),
                        ..
                    }
                ))
            .then_some(fragment.space + 1)
        })
    }

    pub fn scrollnodes(&self) -> BTreeMap<u64, Node> {
        let mut result = BTreeMap::new();
        if let Some(node) = self
            .layout
            .fragments()
            .iter()
            .find_map(domengine::Fragment::target)
        {
            result.insert(0, node);
        }
        for fragment in self.layout.fragments() {
            if matches!(
                fragment.content,
                domengine::Content::Box {
                    scroll: Some(_),
                    ..
                }
            ) && let Some(node) = fragment.target()
            {
                result.insert(fragment.space + 1, node);
            }
        }
        result
    }

    pub fn position(&self, anchor: Anchor) -> Option<Position> {
        let root = self.layout.fragments().iter().find(|fragment| {
            fragment.target() == Some(anchor.node)
                && matches!(
                    fragment.content,
                    domengine::Content::Box {
                        scroll: Some(_),
                        ..
                    }
                )
        })?;
        let origin = root.rect.y;
        let threshold = root.rect.y.saturating_add(anchor.offset);
        let mut previous = None;
        let mut selected: Option<(
            &domengine::Fragment,
            Option<&domengine::Fragment>,
            Option<&domengine::Fragment>,
        )> = None;
        for fragment in self.layout.fragments().iter().filter(|fragment| {
            fragment.parent == Some(root.id)
                && fragment.target().is_some()
                && matches!(fragment.content, domengine::Content::Box { .. })
        }) {
            if let Some((_, _, next)) = &mut selected
                && next.is_none()
            {
                *next = Some(fragment);
            }
            if fragment.rect.y.saturating_add(fragment.rect.height) > threshold
                && selected.as_ref().is_none_or(|(target, _, _)| {
                    (fragment.rect.y, fragment.id) < (target.rect.y, target.id)
                })
            {
                selected = Some((fragment, previous, None));
            }
            previous = Some(fragment);
        }
        let (target, previous, next) = selected?;
        let project = |fragment: &domengine::Fragment| {
            let inset = fragment.rect.y as i128 - origin as i128 - anchor.offset as i128;
            Some(Bookmark {
                target: fragment.target()?,
                inset: i64::try_from(inset).ok()?,
            })
        };
        Some(Position {
            anchor: project(target)?,
            fallbacks: [next.and_then(project), previous.and_then(project)],
        })
    }

    pub fn restore(&self, scroller: Node, target: Node, bookmark: Bookmark) -> Option<usize> {
        let root = self.layout.fragments().iter().find(|fragment| {
            fragment.target() == Some(scroller)
                && matches!(
                    fragment.content,
                    domengine::Content::Box {
                        scroll: Some(_),
                        ..
                    }
                )
        })?;
        let current = self
            .layout
            .fragments()
            .iter()
            .filter(|fragment| {
                fragment.parent == Some(root.id)
                    && fragment.target() == Some(target)
                    && matches!(fragment.content, domengine::Content::Box { .. })
            })
            .min_by_key(|fragment| (fragment.rect.y, fragment.id))?;
        let position = current.rect.y as i128 - root.rect.y as i128;
        usize::try_from((position - i128::from(bookmark.inset)).max(0)).ok()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Anchor {
    pub node: Node,
    pub offset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Bookmark {
    pub target: Node,
    pub inset: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Position {
    pub anchor: Bookmark,
    pub fallbacks: [Option<Bookmark>; 2],
}

pub(super) fn anchor(previous: &Layout, next: &Layout, anchor: Anchor) -> Option<usize> {
    let bookmark = Geometry::new(previous).position(anchor)?.anchor;
    Geometry::new(next).restore(anchor.node, bookmark.target, bookmark)
}

pub(super) fn activate(
    control: &mut super::Control,
    layout: Option<&Layout>,
    scene: &graphics::shared::Scene,
    extent: domengine::Extent,
) -> Option<(u64, usize, super::change::Cause)> {
    let space = control.space?;
    let range = scene.range(space, extent)?;
    let (offset, complete, cause) = match control.action {
        Action::Follow(Edge::End) => (Some(range), false, super::change::Cause::Policy),
        Action::Restore(bookmark) => {
            let desired = layout.and_then(|layout| {
                Geometry::new(layout).restore(control.node, bookmark.target, bookmark)
            });
            (
                desired.map(|offset| offset.min(range)),
                desired.is_some_and(|offset| offset <= range),
                super::change::Cause::Restore,
            )
        }
    };
    if complete {
        control.active = false;
    }
    offset.map(|offset| (space, offset, cause))
}

pub(super) fn unchanged(
    previous: &crate::page::Publication,
    next: &crate::page::Publication,
) -> bool {
    let scene = |publication: &crate::page::Publication| {
        publication
            .scene
            .as_ref()
            .map(|scene| (scene.id(), scene.revision(), scene.extent()))
    };
    let layout = match (&previous.layout, &next.layout) {
        (Some(previous), Some(next)) => std::sync::Arc::ptr_eq(previous, next),
        (None, None) => true,
        _ => false,
    };
    let ownership = next.replay.is_empty() && previous.scrolls == next.scrolls;
    let geometry = scene(previous) == scene(next) && layout;
    ownership && geometry
}
