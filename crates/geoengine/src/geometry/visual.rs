use crate::{Error, Unit};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    start: Unit,
    end: Unit,
}

impl Span {
    pub fn new(start: Unit, end: Unit) -> Result<Self, Error> {
        if end < start {
            return Err(Error("visual span has a negative extent".into()));
        }
        Ok(Self { start, end })
    }

    pub const fn start(self) -> Unit {
        self.start
    }

    pub const fn end(self) -> Unit {
        self.end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coverage {
    leading: Unit,
    trailing: Unit,
    items: usize,
}

impl Coverage {
    pub fn observe(viewport: Span, items: &[Span]) -> Result<Option<Self>, Error> {
        let Some(first) = items.first().copied() else {
            return Ok(None);
        };
        let mut end = first.end;
        for item in &items[1..] {
            if item.start < end {
                return Err(Error("visual items do not form a monotonic flow".into()));
            }
            end = item.end;
        }
        Ok(Some(Self {
            leading: viewport.start.difference(first.start)?,
            trailing: end.difference(viewport.end)?,
            items: items.len(),
        }))
    }

    pub const fn leading(self) -> Unit {
        self.leading
    }

    pub const fn trailing(self) -> Unit {
        self.trailing
    }

    pub const fn items(self) -> usize {
        self.items
    }

    pub fn saturated(self, margin: Unit) -> bool {
        self.leading >= margin && self.trailing >= margin
    }

    pub fn window(viewport: Span, items: &[Span], margin: Unit) -> Result<Range<usize>, Error> {
        let start = viewport.start.difference(margin)?;
        let end = viewport.end.offset(margin)?;
        let mut range: Option<Range<usize>> = None;
        for (index, item) in items.iter().enumerate() {
            if item.end > start && item.start < end {
                range = Some(match range {
                    Some(current) => current.start..index + 1,
                    None => index..index + 1,
                });
            }
        }
        Ok(range.unwrap_or(0..0))
    }
}
