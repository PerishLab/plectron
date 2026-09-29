use super::{Buffer, Selection};
use crate::Fault;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Id(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Emphasis {
    Composition,
    Target,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub emphasis: Emphasis,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub id: Id,
    pub replacement: Selection,
    pub text: String,
    pub selection: Selection,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    tag = "type",
    content = "data",
    rename_all = "lowercase",
    deny_unknown_fields
)]
pub enum Input {
    Update(Composition),
    Mark {
        id: Id,
        text: String,
        selection: Selection,
    },
    Commit {
        id: Id,
        text: String,
    },
    Cancel {
        id: Id,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    Ignored,
    Updated,
    Committed { changed: bool },
    Cancelled,
}

impl Input {
    pub fn id(&self) -> Id {
        match self {
            Self::Update(update) => update.id,
            Self::Mark { id, .. } => *id,
            Self::Commit { id, .. } | Self::Cancel { id } => *id,
        }
    }

    pub fn validate(&self) -> Result<(), Fault> {
        match self {
            Self::Update(update) => update.validate(),
            Self::Mark {
                text, selection, ..
            } => {
                Buffer::validate(text)?;
                if selection.range().end > text.chars().count() {
                    return Err(Fault("composition selection exceeds text length".into()));
                }
                Ok(())
            }
            Self::Commit { text, .. } => Buffer::validate(text),
            Self::Cancel { .. } => Ok(()),
        }
    }
}

impl Composition {
    pub fn validate(&self) -> Result<(), Fault> {
        Buffer::validate(&self.text)?;
        let length = self.text.chars().count();
        if self.selection.range().end > length {
            return Err(Fault("composition selection exceeds text length".into()));
        }
        if self.spans.len() > 64
            || self
                .spans
                .iter()
                .any(|span| span.start > span.end || span.end > length)
        {
            return Err(Fault("invalid composition spans".into()));
        }
        Ok(())
    }
}
