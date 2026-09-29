use super::space::Index;
use super::{Item, Space};
use crate::shared::Fault;

pub fn check(items: &[Item]) -> Result<Index, Fault> {
    let mut budget = Budget {
        size: 0,
        groups: Index::default(),
    };
    budget.walk(items, (0, 0))?;
    Ok(budget.groups)
}
struct Budget {
    size: usize,
    groups: Index,
}
impl Budget {
    fn walk(&mut self, items: &[Item], (depth, parent): (usize, usize)) -> Result<(), Fault> {
        if depth > 32 {
            return Err(Fault("scene depth budget exhausted".into()));
        }
        self.size = self.size.saturating_add(items.len());
        for item in items {
            match item {
                Item::Draw { record, .. } => {
                    self.size = self.size.saturating_add(record.cost() - 1)
                }
                Item::Group {
                    space,
                    opacity,
                    items,
                } => {
                    spacecheck(space)?;
                    let index = self.groups.insert(space, *opacity, parent)?;
                    self.walk(items, (depth + 1, index))?;
                    self.groups.finish(index);
                }
                Item::Fill { color, .. } => colorcheck(color.unwrap_or(0))?,
                Item::Text {
                    font,
                    glyphs,
                    color,
                    ..
                } => {
                    font.validate()?;
                    colorcheck(*color)?;
                    self.size = self.size.saturating_add(glyphs.len());
                    glyphs
                        .iter()
                        .try_for_each(|glyph| font.advance(glyph.id).map(|_| ()))?;
                }
            }
            if self.size > 65_536 {
                return Err(Fault("scene budget exhausted".into()));
            }
        }
        Ok(())
    }
}
pub(super) fn spacecheck(space: &Space) -> Result<(), Fault> {
    if space
        .translation
        .iter()
        .any(|value| value.unsigned_abs() > 1_048_576)
        || space.scroll.is_some_and(|range| range > 1_048_576)
    {
        return Err(Fault("invalid spatial range".into()));
    }
    if space.scroll.is_some() && space.clip.is_none() {
        return Err(Fault("scroll space requires a clip".into()));
    }
    Ok(())
}
pub(super) fn colorcheck(color: u32) -> Result<(), Fault> {
    if color > 0xffffff {
        return Err(Fault("scene colors must be 24-bit sRGB".into()));
    }
    Ok(())
}
