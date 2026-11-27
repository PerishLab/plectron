use super::tree::Branch;
use crate::layout::measure::View;
use crate::{Fault, Local, Node, Rect};
use graphics::shared::Frame;
use std::ops::Range;

impl Branch {
    pub(in crate::layout) fn find(
        &self,
        (base, clip): ([usize; 2], Rect),
        admit: &mut impl FnMut(&Branch) -> bool,
        predicate: &mut impl FnMut(&Local, Rect) -> bool,
    ) -> Option<&Local> {
        let mut links = self.links.iter().rev().peekable();
        for index in (0..=self.fragments.len()).rev() {
            while links.peek().is_some_and(|link| link.at == index) {
                let link = links.next().unwrap();
                let origin = std::array::from_fn(|axis| base[axis] + link.offset[axis]);
                let scope = link.parent.map_or(clip, |p| {
                    shift(self.fragments[p].context(), base).intersect(clip)
                });
                if admit(&link.branch)
                    && let Some(found) = link.branch.find((origin, scope), admit, predicate)
                {
                    return Some(found);
                }
            }
            if index == 0 {
                break;
            }
            let p = &self.fragments[index - 1];
            if p.local.target.is_some()
                && predicate(
                    &p.local,
                    shift(p.rect().intersect(p.clip), base).intersect(clip),
                )
            {
                return Some(&p.local);
            }
        }
        None
    }
    pub(in crate::layout) fn targets(
        &self,
        frame: &graphics::shared::Frame,
        x: usize,
        y: usize,
    ) -> bool {
        let Some(targets) = &self.targets else {
            return true;
        };
        targets.iter().any(|target| {
            frame
                .mapping(target.space)
                .is_some_and(|mapping| mapping.map(target.bounds).contains(x, y))
        })
    }
}
fn shift(rect: Rect, base: [usize; 2]) -> Rect {
    Rect {
        x: rect.x + base[0],
        y: rect.y + base[1],
        ..rect
    }
}
impl crate::Layout {
    pub(in crate::layout) fn scope(&self) -> ([usize; 2], Rect) {
        (
            [0; 2],
            Rect {
                x: 0,
                y: 0,
                width: super::super::constraints::LIMIT,
                height: super::super::constraints::LIMIT,
            },
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coverage {
    pub revision: u64,
    pub leading: i32,
    pub trailing: i32,
    pub items: usize,
    pub visible: Range<usize>,
    pub inset: i32,
}

impl super::super::Layout {
    pub fn coverage(
        &self,
        frame: &Frame,
        node: Node,
        margin: usize,
    ) -> Result<Option<Coverage>, Fault> {
        let root = self.fragments().iter().find(|fragment| {
            fragment.target() == Some(node)
                && matches!(
                    fragment.content,
                    super::Content::Box {
                        scroll: Some(_),
                        ..
                    }
                )
        });
        let Some(root) = root else {
            return Err(Fault("visual observer requires a scroll container".into()));
        };
        let space = root.space + 1;
        let mapping = frame
            .mapping(space)
            .ok_or_else(|| Fault("visual observer scroll space is absent".into()))?;
        let unit = |value: i64| {
            i32::try_from(value)
                .map(geoengine::Unit::new)
                .map_err(|_| Fault("visual observer coordinate overflow".into()))
        };
        let viewport = geoengine::Span::new(
            unit(mapping.clip.y as i64)?,
            unit(mapping.clip.y.saturating_add(mapping.clip.height) as i64)?,
        )
        .map_err(|error| Fault(error.to_string()))?;
        let mut spans = Vec::new();
        for fragment in self.fragments().iter().filter(|fragment| {
            fragment.parent == Some(root.id)
                && fragment.target().is_some()
                && matches!(fragment.content, super::Content::Box { .. })
        }) {
            let item = self
                .mapping(frame, fragment)
                .ok_or_else(|| Fault("visual item has no geometry mapping".into()))?;
            let start = fragment.rect.y as i64 + item.translation[1];
            let end = start + fragment.rect.height as i64;
            spans.push(
                geoengine::Span::new(unit(start)?, unit(end)?)
                    .map_err(|error| Fault(error.to_string()))?,
            );
        }
        let Some(coverage) = geoengine::Coverage::observe(viewport, &spans)
            .map_err(|error| Fault(error.to_string()))?
        else {
            return Ok(None);
        };
        let visible = geoengine::Coverage::window(
            viewport,
            &spans,
            geoengine::Unit::positive(margin).map_err(|error| Fault(error.to_string()))?,
        )
        .map_err(|error| Fault(error.to_string()))?;
        let inset = spans
            .get(visible.start)
            .and_then(|span| span.start().difference(viewport.start()).ok())
            .map_or(0, geoengine::Unit::value);
        Ok(Some(Coverage {
            revision: self.revision,
            leading: coverage.leading().value(),
            trailing: coverage.trailing().value(),
            items: coverage.items(),
            visible,
            inset,
        }))
    }

    pub fn caret(&self, node: Node, position: usize) -> Option<Rect> {
        let (view, origin) = editor(self.snapshot.view(), [0, 0], node)?;
        let text = view.material().text()?;
        let lines = view.text?;
        let offset = byte(text, position);
        let (row, x) = view.caret(offset);
        let line = lines.ranges.get(row)?;
        let remaining = view.size()[0].saturating_sub(line.width.ceil());
        let alignment = view
            .material()
            .style()
            .typography
            .align
            .offset(remaining, line.direction % 2 == 1)
            .saturating_sub(usize::from(line.direction % 2 == 1));
        Some(Rect {
            x: origin[0] + alignment + x,
            y: origin[1] + line.y,
            width: 1,
            height: line.height,
        })
    }

    pub fn locate(&self, node: Node, point: [usize; 2]) -> Option<usize> {
        let (view, origin) = editor(self.snapshot.view(), [0, 0], node)?;
        let text = view.material().text()?;
        let local = [
            point[0].saturating_sub(origin[0]),
            point[1].saturating_sub(origin[1]),
        ];
        let offset = view.hit(local);
        Some(text[..offset.min(text.len())].chars().count())
    }

    pub fn vertical(&self, node: Node, position: usize, down: bool) -> Option<usize> {
        self.step(node, position, (down, None))
            .map(|(next, _)| next)
    }

    pub fn step(
        &self,
        node: Node,
        position: usize,
        motion: (bool, Option<usize>),
    ) -> Option<(usize, usize)> {
        let (view, _) = editor(self.snapshot.view(), [0, 0], node)?;
        let text = view.material().text()?;
        let offset = byte(text, position);
        let (next, column) = view.vertical(offset, motion.0, motion.1);
        Some((text[..next.min(text.len())].chars().count(), column))
    }
}

fn editor<'a>(view: View<'a>, origin: [usize; 2], node: Node) -> Option<(View<'a>, [usize; 2])> {
    if view.text.is_some() {
        let material = view.material();
        if material.editor().is_some() && material.target() == node {
            let point = [origin[0] + view.offset[0], origin[1] + view.offset[1]];
            return Some((view, point));
        }
        return None;
    }
    if view.deferred() {
        return None;
    }
    let style = view.material().style();
    let inner = [
        origin[0] + view.offset[0] + style.margin + style.padding,
        origin[1] + view.offset[1] + style.margin + style.padding,
    ];
    for child in view.children() {
        if let Some(found) = editor(child, inner, node) {
            return Some(found);
        }
    }
    None
}

fn byte(text: &str, position: usize) -> usize {
    text.char_indices()
        .nth(position)
        .map_or(text.len(), |(index, _)| index)
}
