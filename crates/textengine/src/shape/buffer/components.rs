use std::ops::Range;

#[derive(Clone)]
pub(in crate::shape) enum Components {
    One(Range<usize>),
    Many(Vec<Range<usize>>),
}

impl Components {
    pub fn entries(&self) -> &[Range<usize>] {
        match self {
            Self::One(range) => std::slice::from_ref(range),
            Self::Many(ranges) => ranges,
        }
    }
    pub fn edit(&mut self) -> &mut [Range<usize>] {
        match self {
            Self::One(range) => std::slice::from_mut(range),
            Self::Many(ranges) => ranges,
        }
    }
    pub fn bytes(&self) -> usize {
        match self {
            Self::One(_) => 0,
            Self::Many(ranges) => std::mem::size_of_val(ranges.as_slice()),
        }
    }
}
