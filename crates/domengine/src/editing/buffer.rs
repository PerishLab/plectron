use super::{
    Edit, Selection,
    ime::{Composition, Effect, Input},
};
use crate::Fault;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Buffer {
    text: String,
    selection: Selection,
    composition: Option<Composition>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Projection {
    pub text: String,
    pub offsets: [usize; 2],
}

impl Buffer {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn caret(&self) -> usize {
        self.selection.focus
    }

    pub fn offset(&self) -> usize {
        self.byte(self.selection.focus)
    }

    pub fn selection(&self) -> Selection {
        self.selection
    }

    pub fn composition(&self) -> Option<&Composition> {
        self.composition.as_ref()
    }

    pub fn offsets(&self) -> [usize; 2] {
        [self.byte(self.selection.anchor), self.offset()]
    }

    pub fn projection(&self) -> Projection {
        let Some(composition) = &self.composition else {
            return Projection {
                text: self.text.clone(),
                offsets: self.offsets(),
            };
        };
        let range = composition.replacement.range();
        let bytes = self.byte(range.start)..self.byte(range.end);
        let mut text =
            String::with_capacity(self.text.len() - bytes.len() + composition.text.len());
        text.push_str(&self.text[..bytes.start]);
        text.push_str(&composition.text);
        text.push_str(&self.text[bytes.end..]);
        let start = bytes.start;
        Projection {
            offsets: [
                start + byte(&composition.text, composition.selection.anchor),
                start + byte(&composition.text, composition.selection.focus),
            ],
            text,
        }
    }

    pub(super) fn validate(text: &str) -> Result<(), Fault> {
        if text.len() > 4096 || text.chars().any(char::is_control) {
            return Err(Fault(
                "input requires at most 4096 bytes of single-line text".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn replace(&mut self, text: &str) -> Result<(), Fault> {
        Self::validate(text)?;
        self.composition = None;
        if self.text != text {
            self.text = text.into();
            self.selection = Selection::caret(text.chars().count());
        }
        Ok(())
    }

    pub(super) fn select(&mut self, selection: Selection) -> Result<(), Fault> {
        if selection.range().end > self.text.chars().count() {
            return Err(Fault("selection exceeds scalar text length".into()));
        }
        self.composition = None;
        self.selection = selection;
        Ok(())
    }

    pub(super) fn apply(&mut self, edit: &Edit) -> Result<bool, Fault> {
        edit.validate()?;
        self.composition = None;
        let old = self.text.clone();
        let length = self.text.chars().count();
        match edit {
            Edit::Insert(text) => {
                self.splice(self.selection.range(), text)?;
            }
            Edit::Backward | Edit::Forward => {
                self.erase(matches!(edit, Edit::Backward), length)?;
            }
            Edit::Move { direction, extend } => self.selection.advance(*direction, *extend, length),
            Edit::Select(selection) => self.select(*selection)?,
            Edit::All => {
                self.selection = Selection {
                    anchor: 0,
                    focus: length,
                }
            }
            _ => {}
        }
        Ok(old != self.text)
    }

    pub(super) fn compose(&mut self, input: &Input) -> Result<Effect, Fault> {
        input.validate()?;
        match input {
            Input::Update(update) => self.update(update),
            Input::Mark {
                id,
                text,
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
                    text: text.clone(),
                    selection: *selection,
                    spans: Vec::new(),
                })
            }
            Input::Commit { id, text } => self.commit(*id, text),
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
        self.admit(update.replacement, &update.text)?;
        self.composition = Some(update.clone());
        Ok(Effect::Updated)
    }

    fn commit(&mut self, id: super::ime::Id, text: &str) -> Result<Effect, Fault> {
        let Some(active) = &self.composition else {
            return Ok(Effect::Ignored);
        };
        if active.id != id {
            return Ok(Effect::Ignored);
        }
        let replacement = active.replacement;
        self.admit(replacement, text)?;
        self.composition = None;
        let old = self.text.clone();
        self.splice(replacement.range(), text)?;
        Ok(Effect::Committed {
            changed: old != self.text,
        })
    }

    pub(super) fn cancel(&mut self) -> bool {
        self.composition.take().is_some()
    }

    fn admit(&self, selection: Selection, text: &str) -> Result<(), Fault> {
        let range = selection.range();
        if range.end > self.text.chars().count() {
            return Err(Fault("composition replacement exceeds text length".into()));
        }
        let bytes = self.byte(range.start)..self.byte(range.end);
        if self.text.len() - bytes.len() + text.len() > 4096 {
            return Err(Fault("input text budget exhausted".into()));
        }
        Ok(())
    }

    fn erase(&mut self, backward: bool, length: usize) -> Result<(), Fault> {
        let mut range = self.selection.range();
        if range.is_empty() {
            if backward {
                range.start = range.start.saturating_sub(1);
            } else {
                range.end = (range.end + 1).min(length);
            }
        }
        self.splice(range, "")
    }

    fn splice(&mut self, range: std::ops::Range<usize>, text: &str) -> Result<(), Fault> {
        let bytes = self.byte(range.start)..self.byte(range.end);
        if self.text.len() - bytes.len() + text.len() > 4096 {
            return Err(Fault("input text budget exhausted".into()));
        }
        self.text.replace_range(bytes, text);
        self.selection = Selection::caret(range.start + text.chars().count());
        Ok(())
    }

    fn byte(&self, position: usize) -> usize {
        self.text
            .char_indices()
            .nth(position)
            .map_or(self.text.len(), |(byte, _)| byte)
    }
}

fn byte(text: &str, position: usize) -> usize {
    text.char_indices()
        .nth(position)
        .map_or(text.len(), |(byte, _)| byte)
}
