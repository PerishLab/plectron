use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Grammar,
    Syntax,
    Budget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub kind: Kind,
    pub offset: usize,
    pub note: String,
}

impl Error {
    pub fn new(kind: Kind, offset: usize, note: impl Into<String>) -> Self {
        Self {
            kind,
            offset,
            note: note.into(),
        }
    }

    pub(crate) fn grammar(note: impl Into<String>) -> Self {
        Self::new(Kind::Grammar, 0, note)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} at byte {}: {}",
            self.kind, self.offset, self.note
        )
    }
}

impl std::error::Error for Error {}
