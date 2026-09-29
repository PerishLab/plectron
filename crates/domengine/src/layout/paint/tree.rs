use super::{group, recording::Recording};
use crate::{Branch, Content, Fault, Item};
use graphics::shared::Space;

struct Cursor<'a> {
    branch: &'a Branch,
    fragment: usize,
    link: usize,
}
impl Branch {
    pub(super) fn compose(&self, draw: &mut impl Recording) -> Result<Item, Fault> {
        let mut cursor = Cursor {
            branch: self,
            fragment: 0,
            link: 0,
        };
        let item = cursor.append(draw)?;
        if cursor.fragment != self.fragments.len() || cursor.link != self.links.len() {
            return Err(Fault("fragment tree is not preorder".into()));
        }
        Ok(item)
    }
}
impl Cursor<'_> {
    fn parent(&self) -> Option<Option<usize>> {
        if let Some(link) = self.branch.links.get(self.link)
            && link.at == self.fragment
        {
            return Some(link.parent);
        }
        self.branch.fragments.get(self.fragment).map(|p| p.parent)
    }
    fn record(
        &mut self,
        parent: Option<usize>,
        draw: &mut impl Recording,
        items: &mut Vec<Item>,
    ) -> Result<(), Fault> {
        while self.parent() == Some(parent) {
            if let Some(link) = self.branch.links.get(self.link)
                && link.at == self.fragment
            {
                let basis = parent.map_or([0; 2], |p| self.branch.fragments[p].origin);
                let offset =
                    std::array::from_fn(|axis| link.offset[axis] as i64 - basis[axis] as i64);
                items.push(draw.place(&link.branch, offset)?);
                self.link += 1;
            } else {
                if matches!(
                    self.branch.fragments[self.fragment].local.content,
                    Content::Line
                ) {
                    let index = self.fragment;
                    self.fragment += 1;
                    self.record(Some(index), draw, items)?;
                } else {
                    items.push(self.append(draw)?);
                }
            }
        }
        Ok(())
    }
    fn append(&mut self, draw: &mut impl Recording) -> Result<Item, Fault> {
        let index = self.fragment;
        self.fragment += 1;
        let placed = &self.branch.fragments[index];
        let fragment = &placed.local;
        let mut children = Vec::new();
        self.record(Some(index), draw, &mut children)?;
        let offset = placed.offset.map(|v| v as i64);
        Ok(match &fragment.content {
            Content::Box { .. } => group((fragment, offset), draw.draw(fragment)?, children),
            Content::Inline { translation } => Item::Group {
                space: Space {
                    id: fragment.space,
                    translation: std::array::from_fn(|axis| translation[axis] + offset[axis]),
                    clip: None,
                    scroll: None,
                },
                opacity: 255,
                items: children.into(),
            },
            Content::Line => unreachable!(),
            _ => draw.draw(fragment)?,
        })
    }
}
