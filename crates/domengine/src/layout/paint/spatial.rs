use crate::{Branch, Content, Fault};
use graphics::shared::{Space, Spatial};

pub(super) fn compose(branch: &Branch) -> Result<Vec<Spatial>, Fault> {
    let mut nodes = Vec::new();
    append(branch, 0, [0; 2], &mut nodes)?;
    Ok(nodes)
}

fn append(
    branch: &Branch,
    parent: usize,
    shift: [i64; 2],
    nodes: &mut Vec<Spatial>,
) -> Result<(), Fault> {
    let mut cursor = Cursor {
        branch,
        fragment: 0,
        link: 0,
        shift,
    };
    cursor.record(None, parent, nodes)?;
    if cursor.fragment != branch.fragments.len() || cursor.link != branch.links.len() {
        return Err(Fault("fragment tree is not preorder".into()));
    }
    Ok(())
}

struct Cursor<'a> {
    branch: &'a Branch,
    fragment: usize,
    link: usize,
    shift: [i64; 2],
}

impl Cursor<'_> {
    fn parent(&self) -> Option<Option<usize>> {
        if let Some(link) = self.branch.links.get(self.link)
            && link.at == self.fragment
        {
            return Some(link.parent);
        }
        self.branch
            .fragments
            .get(self.fragment)
            .map(|placed| placed.parent)
    }

    fn record(
        &mut self,
        owner: Option<usize>,
        parent: usize,
        nodes: &mut Vec<Spatial>,
    ) -> Result<(), Fault> {
        while self.parent() == Some(owner) {
            if self
                .branch
                .links
                .get(self.link)
                .is_some_and(|link| link.at == self.fragment)
            {
                self.independent(owner, parent, nodes)?;
                continue;
            }
            if matches!(
                self.branch.fragments[self.fragment].local.content,
                Content::Line
            ) {
                let index = self.fragment;
                self.fragment += 1;
                self.record(Some(index), parent, nodes)?;
            } else {
                self.fragment(parent, nodes)?;
            }
        }
        Ok(())
    }

    fn independent(
        &mut self,
        owner: Option<usize>,
        parent: usize,
        nodes: &mut Vec<Spatial>,
    ) -> Result<(), Fault> {
        let link = &self.branch.links[self.link];
        let basis = owner.map_or([0; 2], |index| self.branch.fragments[index].origin);
        let shift = std::array::from_fn(|axis| link.offset[axis] as i64 - basis[axis] as i64);
        append(&link.branch, parent, shift, nodes)?;
        self.link += 1;
        Ok(())
    }

    fn fragment(&mut self, parent: usize, nodes: &mut Vec<Spatial>) -> Result<(), Fault> {
        let index = self.fragment;
        self.fragment += 1;
        let placed = &self.branch.fragments[index];
        let fragment = &placed.local;
        let offset = placed.offset.map(|value| value as i64);
        let shift = if index == 0 { self.shift } else { [0; 2] };
        match &fragment.content {
            Content::Box {
                translation,
                opacity,
                viewport,
                scroll,
                ..
            } => {
                let outer = push(
                    nodes,
                    parent,
                    Space {
                        id: fragment.space,
                        translation: std::array::from_fn(|axis| {
                            translation[axis] + offset[axis] + shift[axis]
                        }),
                        clip: None,
                        scroll: None,
                    },
                    *opacity,
                );
                let content = push(
                    nodes,
                    outer,
                    Space {
                        id: fragment.space + 1,
                        translation: [0, 0],
                        clip: *viewport,
                        scroll: *scroll,
                    },
                    255,
                );
                self.record(Some(index), content, nodes)?;
                finish(nodes, content);
                finish(nodes, outer);
            }
            Content::Inline { translation } => {
                let group = push(
                    nodes,
                    parent,
                    Space {
                        id: fragment.space,
                        translation: std::array::from_fn(|axis| {
                            translation[axis] + offset[axis] + shift[axis]
                        }),
                        clip: None,
                        scroll: None,
                    },
                    255,
                );
                self.record(Some(index), group, nodes)?;
                finish(nodes, group);
            }
            Content::Line => unreachable!(),
            _ => self.record(Some(index), parent, nodes)?,
        }
        Ok(())
    }
}

fn push(nodes: &mut Vec<Spatial>, parent: usize, space: Space, opacity: u8) -> usize {
    nodes.push(Spatial {
        space,
        opacity,
        parent,
        descendants: 0,
    });
    nodes.len()
}

fn finish(nodes: &mut [Spatial], index: usize) {
    nodes[index - 1].descendants = nodes.len() - index;
}
