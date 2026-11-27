use super::{Buffer, Selection, text::Policy};
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
                text: value,
                selection,
                ..
            } => {
                Policy(true).validate(value)?;
                if selection.range().end > value.chars().count() {
                    return Err(Fault("composition selection exceeds text length".into()));
                }
                Ok(())
            }
            Self::Commit { text: value, .. } => Policy(true).validate(value),
            Self::Cancel { .. } => Ok(()),
        }
    }
}

impl Composition {
    pub fn validate(&self) -> Result<(), Fault> {
        Policy(true).validate(&self.text)?;
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

impl Buffer {
    pub(super) fn compose(&mut self, input: &Input) -> Result<Effect, Fault> {
        input.validate()?;
        self.column = None;
        match input {
            Input::Update(update) => self.update(update),
            Input::Mark {
                id,
                text: marked,
                selection,
            } => {
                let replacement = self
                    .composition
                    .as_ref()
                    .filter(|active| active.id == *id)
                    .map_or(self.selection, |active| active.replacement);
                self.update(&Composition {
                    id: *id,
                    replacement,
                    text: marked.clone(),
                    selection: *selection,
                    spans: Vec::new(),
                })
            }
            Input::Commit { id, text: value } => self.commit(*id, value),
            Input::Cancel { id } => {
                if self
                    .composition
                    .as_ref()
                    .is_none_or(|active| active.id != *id)
                {
                    return Ok(Effect::Ignored);
                }
                self.composition = None;
                Ok(Effect::Cancelled)
            }
        }
    }

    fn update(&mut self, update: &Composition) -> Result<Effect, Fault> {
        if let Some(active) = &self.composition {
            if active.id != update.id {
                return Ok(Effect::Ignored);
            }
            if active.replacement != update.replacement {
                return Err(Fault("composition target changed during session".into()));
            }
        }
        let policy = Policy(self.multiline);
        policy.validate(&update.text)?;
        let normalized = policy.prepare(&update.text);
        self.admit(update.replacement, &normalized)?;
        let mut stored = update.clone();
        if stored.text != normalized.as_ref() {
            stored.text = normalized.into_owned();
        }
        self.composition = Some(stored);
        Ok(Effect::Updated)
    }

    fn commit(&mut self, id: Id, value: &str) -> Result<Effect, Fault> {
        let Some(active) = &self.composition else {
            return Ok(Effect::Ignored);
        };
        if active.id != id {
            return Ok(Effect::Ignored);
        }
        let policy = Policy(self.multiline);
        policy.validate(value)?;
        let normalized = policy.prepare(value);
        let replacement = active.replacement;
        self.admit(replacement, &normalized)?;
        self.composition = None;
        let old = self.text.clone();
        self.splice(replacement.range(), &normalized)?;
        Ok(Effect::Committed {
            changed: old != self.text,
        })
    }

    fn admit(&self, selection: Selection, value: &str) -> Result<(), Fault> {
        let range = selection.range();
        if range.end > self.text.chars().count() {
            return Err(Fault("composition replacement exceeds text length".into()));
        }
        let bytes = self.byte(range.start)..self.byte(range.end);
        if self.text.len() - bytes.len() + value.len() > self.limit() {
            return Err(Fault("input text budget exhausted".into()));
        }
        Ok(())
    }
}
