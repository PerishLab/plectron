use crate::Fault;
use std::{ops::Range, sync::Arc};

pub(in crate::shape) struct Source {
    text: Arc<str>,
    boundaries: Vec<usize>,
}
#[derive(Clone, Copy)]
pub(in crate::shape) struct View<'a> {
    text: &'a str,
    boundaries: &'a [usize],
    origin: usize,
}
impl Source {
    pub fn new(text: &str) -> Self {
        Self {
            text: text.into(),
            boundaries: std::iter::once(0)
                .chain(
                    crate::unicode::graphemes(text)
                        .iter()
                        .map(|range| range.end),
                )
                .collect(),
        }
    }
    pub fn view(&self) -> View<'_> {
        View {
            text: &self.text,
            boundaries: &self.boundaries,
            origin: 0,
        }
    }
    pub fn bytes(&self) -> usize {
        self.text.len() + std::mem::size_of_val(self.boundaries.as_slice())
    }
}
impl<'a> View<'a> {
    pub fn text(self) -> &'a str {
        self.text
    }
    pub fn origin(self) -> usize {
        self.origin
    }
    pub fn contains(self, offset: usize) -> bool {
        self.origin
            .checked_add(offset)
            .is_some_and(|offset| self.boundaries.binary_search(&offset).is_ok())
    }
    pub fn slice(self, range: Range<usize>) -> Result<Self, Fault> {
        let invalid = || Fault("shaping range must follow source graphemes".into());
        let text = self.text.get(range.clone()).ok_or_else(invalid)?;
        if range == (0..self.text.len()) {
            return Ok(self);
        }
        let origin = self.origin + range.start;
        let start = self
            .boundaries
            .binary_search(&origin)
            .map_err(|_| invalid())?;
        let end = self
            .boundaries
            .binary_search(&(self.origin + range.end))
            .map_err(|_| invalid())?;
        Ok(Self {
            text,
            boundaries: &self.boundaries[start..=end],
            origin,
        })
    }
}
