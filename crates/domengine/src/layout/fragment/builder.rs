use super::tree::Link;
use crate::layout::{constraints::LIMIT, measure::View};
use crate::{Content, Fault, Fragment, Local, Placement, Rect};

pub(in crate::layout) struct Builder<'a> {
    pub placements: Vec<Placement<Local>>,
    pub links: Vec<Link>,
    pub projection: &'a mut crate::layout::cache::Projection,
    pub owner: crate::Node,
    pub previous: Option<std::sync::Arc<crate::Branch>>,
    pub query: usize,
}
impl Builder<'_> {
    pub(in crate::layout) fn push(&mut self, mut fragment: Fragment) -> Result<usize, Fault> {
        fragment.id = self.placements.len();
        let parent = fragment
            .parent
            .map_or([0; 2], |id| self.placements[id].origin);
        self.place(Placement::new(fragment, parent))
    }
    pub(in crate::layout) fn place(&mut self, fragment: Placement<Local>) -> Result<usize, Fault> {
        let rect = fragment.rect();
        if self.placements.len() >= 65_536
            || rect.x.saturating_add(rect.width) > LIMIT
            || rect.y.saturating_add(rect.height) > LIMIT
        {
            return Err(Fault("fragment budget exhausted".into()));
        }
        let id = fragment.id;
        self.placements.push(fragment);
        Ok(id)
    }
    pub(in crate::layout) fn project(
        &mut self,
        measured: &View<'_>,
        origin: [usize; 2],
        context: (Option<usize>, Rect),
    ) -> Result<(), Fault> {
        if measured.text.is_some() {
            return self.lines(measured, origin, context);
        }
        let material = measured.material();
        let style = material.style();
        if style.overflow == crate::style::Overflow::Auto && !measured.clipped() {
            return Err(Fault("scroll container requires definite height".into()));
        }
        let rect = Rect {
            x: origin[0] + measured.offset[0] + style.margin,
            y: origin[1] + measured.offset[1] + style.margin,
            width: measured.size()[0] + style.padding * 2,
            height: measured.size()[1] + style.padding * 2,
        };
        let inner = Rect {
            x: rect.x + style.padding,
            y: rect.y + style.padding,
            width: measured.size()[0],
            height: measured.size()[1],
        };
        let id = self.push(Fragment {
            id: 0,
            space: material.node().id() as u64 * 3 + 1,
            parent: context.0,
            node: Some(material.node().id()),
            target: Some(material.target()),
            rect,
            clip: context.1,
            content: Content::Box {
                color: style.background,
                translation: style.translation,
                opacity: style.opacity,
                viewport: measured.clipped().then_some(inner),
                scroll: (style.overflow == crate::style::Overflow::Auto)
                    .then_some(measured.overflow()[1].saturating_sub(inner.height)),
            },
        })?;
        let clip = if measured.clipped() {
            context.1.intersect(inner)
        } else {
            context.1
        };
        if measured.deferred() {
            return Ok(());
        }
        if material.picture().is_some() {
            self.picture(measured, inner, id)?;
        }
        for child in measured.children() {
            self.emit(&child, [inner.x, inner.y], (Some(id), clip))?;
        }
        Ok(())
    }
    pub(in crate::layout) fn lines(
        &mut self,
        measured: &View<'_>,
        origin: [usize; 2],
        context: (Option<usize>, Rect),
    ) -> Result<(), Fault> {
        let lines = measured.text.as_ref().expect("measured text");
        let material = measured.material();
        let editing = material.editor().is_some();
        let space = context.0.map_or(0, |parent| {
            self.placements[parent].local.space + if editing { 2 } else { 1 }
        });
        for (index, line) in lines.ranges.iter().enumerate() {
            let remaining = measured.size()[0].saturating_sub(line.width.ceil());
            let alignment = material
                .style()
                .typography
                .align
                .offset(remaining, line.direction % 2 == 1)
                .saturating_sub(usize::from(editing && line.direction % 2 == 1));
            let rect = Rect {
                x: origin[0] + measured.offset[0] + alignment,
                y: origin[1] + measured.offset[1] + line.y,
                width: line.width.ceil(),
                height: line.height,
            };
            let parent = self.push(Fragment {
                id: 0,
                space,
                parent: context.0,
                node: None,
                target: None,
                rect,
                clip: context.1,
                content: self.line(measured, context.0),
            })?;
            self.selection(measured, parent)?;
            self.glyphs(measured, index, parent)?;
            if let Some(editor) = material.editor()
                && editor.active
            {
                self.caret(measured, editor.offsets[1], parent)?;
            }
        }
        Ok(())
    }
}
impl Builder<'_> {
    pub(super) fn line(&self, measured: &View<'_>, parent: Option<usize>) -> Content {
        let Some(editor) = measured.material().editor() else {
            return Content::Line;
        };
        let viewport = parent.and_then(|parent| match self.placements[parent].local.content {
            Content::Box { viewport, .. } => {
                viewport.map(|area| self.placements[parent].area(area))
            }
            _ => None,
        });
        let offset = viewport.map_or(0, |rect| {
            measured
                .position(editor.offsets[1])
                .saturating_add(1)
                .saturating_sub(rect.width)
        });
        Content::Inline {
            translation: [-(offset as i64), 0],
        }
    }

    pub(super) fn selection(&mut self, measured: &View<'_>, parent: usize) -> Result<(), Fault> {
        let Some(editor) = measured.material().editor() else {
            return Ok(());
        };
        if !editor.active || editor.offsets[0] == editor.offsets[1] {
            return Ok(());
        }
        let range =
            editor.offsets[0].min(editor.offsets[1])..editor.offsets[0].max(editor.offsets[1]);
        for interval in measured.intervals(range) {
            self.mark(parent, interval, Content::Selection { color: 0x9fc5f8 })?;
        }
        Ok(())
    }

    pub(super) fn caret(
        &mut self,
        measured: &View<'_>,
        offset: usize,
        parent: usize,
    ) -> Result<(), Fault> {
        let position = measured.position(offset);
        if let Some(caret) = measured.material().editor().and_then(|editor| editor.caret) {
            let line = &self.placements[parent];
            self.push(Fragment {
                id: 0,
                space: caret.node.id() as u64 * 3 + 1,
                parent: Some(parent),
                node: Some(caret.node.id()),
                target: Some(measured.material().target()),
                rect: Rect {
                    x: line.rect().x + position,
                    y: line.rect().y,
                    width: caret.width,
                    height: caret.height.unwrap_or(line.rect().height),
                },
                clip: line.clip,
                content: Content::Box {
                    color: Some(caret.color),
                    translation: caret.translation,
                    opacity: caret.opacity,
                    viewport: None,
                    scroll: None,
                },
            })?;
            return Ok(());
        }
        self.mark(
            parent,
            position..position + 1,
            Content::Caret {
                color: measured.material().style().color,
            },
        )
    }

    fn mark(
        &mut self,
        parent: usize,
        range: std::ops::Range<usize>,
        content: Content,
    ) -> Result<(), Fault> {
        let line = &self.placements[parent];
        self.push(Fragment {
            id: 0,
            space: line.local.space,
            parent: Some(parent),
            node: None,
            target: None,
            rect: Rect {
                x: line.rect().x + range.start,
                y: line.rect().y,
                width: range.len(),
                height: line.rect().height,
            },
            clip: line.clip,
            content,
        })?;
        Ok(())
    }
}
