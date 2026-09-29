use super::{Point, Size, Unit};
use crate::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Child {
    outer: Size,
    coverage: Size,
}

impl Child {
    pub const fn new(outer: Size, coverage: Size) -> Self {
        Self { outer, coverage }
    }
}

pub struct Block {
    children: Vec<Child>,
    limit: Unit,
}

impl Block {
    pub fn new(children: Vec<Child>, limit: Unit) -> Self {
        Self { children, limit }
    }

    pub fn resolve(self) -> Result<Flow, Error> {
        let mut placements = Vec::with_capacity(self.children.len());
        let mut width = Unit::ZERO;
        let mut height = Unit::ZERO;
        let mut overflow = Size::default();
        for child in self.children {
            placements.push(Point {
                x: Unit::ZERO,
                y: height,
            });
            width = width.max(child.outer.width);
            overflow.width = overflow.width.max(child.coverage.width);
            overflow.height = overflow.height.max(height.offset(child.coverage.height)?);
            height = height.offset(child.outer.height)?;
            if height > self.limit {
                return Err(Error("block size budget exhausted".into()));
            }
        }
        Ok(Flow {
            placements,
            size: Size { width, height },
            overflow,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flow {
    placements: Vec<Point>,
    size: Size,
    overflow: Size,
}

impl Flow {
    pub fn placements(&self) -> &[Point] {
        &self.placements
    }

    pub const fn size(&self) -> Size {
        self.size
    }

    pub const fn overflow(&self) -> Size {
        self.overflow
    }
}
