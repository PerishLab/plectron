use crate::{Extent, Fault};

pub(super) const LIMIT: usize = 1_048_576;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Available {
    Definite(usize),
    Min,
    Max,
}
impl Available {
    pub fn definite(self) -> Option<usize> {
        match self {
            Self::Definite(value) => Some(value),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Constraints {
    pub available: [Available; 2],
    pub known: [Option<usize>; 2],
}
impl Constraints {
    pub fn viewport(extent: Extent) -> Self {
        Self {
            available: [
                Available::Definite(extent.width),
                Available::Definite(extent.height),
            ],
            known: [None; 2],
        }
    }
    pub(super) fn check(self) -> Result<(), Fault> {
        if self
            .available
            .iter()
            .filter_map(|value| value.definite())
            .chain(self.known.into_iter().flatten())
            .any(|value| value > LIMIT)
        {
            return Err(Fault("layout constraint budget exhausted".into()));
        }
        Ok(())
    }
}
