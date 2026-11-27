use super::{
    Edit, Selection,
    ime::Composition,
    text::{self, Policy},
};
use crate::Fault;
use serde::Serialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Buffer {
    pub(super) text: String,
    pub(super) selection: Selection,
    pub(super) composition: Option<Composition>,
    #[serde(skip)]
    pub(super) multiline: bool,
    #[serde(skip)]
    pub(super) column: Option<usize>,
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

    pub fn multiline(&self) -> bool {
        self.multiline
    }

    pub fn limit(&self) -> usize {
        Policy(self.multiline).limit()
    }

    pub fn selected(&self) -> &str {
        let range = self.selection.range();
        &self.text[self.byte(range.start)..self.byte(range.end)]
    }

    pub fn sanitize(&self, value: &str) -> String {
        Policy(self.multiline).sanitize(value)
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
        let mut projected =
            String::with_capacity(self.text.len() - bytes.len() + composition.text.len());
        projected.push_str(&self.text[..bytes.start]);
        projected.push_str(&composition.text);
        projected.push_str(&self.text[bytes.end..]);
        let start = bytes.start;
        Projection {
            offsets: [
                start + text::byte(&composition.text, composition.selection.anchor),
                start + text::byte(&composition.text, composition.selection.focus),
            ],
            text: projected,
        }
    }

    pub fn check(&self, edit: &Edit) -> Result<(), Fault> {
        match edit {
            Edit::Insert(value) => Policy(self.multiline).validate(value),
            Edit::Select(selection) if selection.range().end > self.limit() => {
                Err(Fault("selection exceeds input budget".into()))
            }
            Edit::Paste(Some(value)) if value.len() > text::MULTI * 2 => {
                Err(Fault("input text budget exhausted".into()))
            }
            _ => Ok(()),
        }
    }

    pub(super) fn configure(&mut self, multiline: bool) -> Result<bool, Fault> {
        if self.multiline == multiline {
            return Ok(false);
        }
        Policy(multiline).validate(&self.text)?;
        self.multiline = multiline;
        self.column = None;
        Ok(true)
    }

    pub(super) fn replace(&mut self, value: &str) -> Result<(), Fault> {
        let policy = Policy(self.multiline);
        policy.validate(value)?;
        let normalized = policy.prepare(value);
        self.composition = None;
        self.column = None;
        if self.text != normalized.as_ref() {
            self.text = normalized.into_owned();
            self.selection = Selection::caret(self.text.chars().count());
        }
        Ok(())
    }

    pub(super) fn select(&mut self, selection: Selection) -> Result<(), Fault> {
        if selection.range().end > self.text.chars().count() {
            return Err(Fault("selection exceeds scalar text length".into()));
        }
        self.composition = None;
        self.column = None;
        self.selection = selection;
        Ok(())
    }

    pub(super) fn apply(&mut self, edit: &Edit) -> Result<bool, Fault> {
        edit.validate()?;
        self.check(edit)?;
        self.composition = None;
        let old = self.text.clone();
        let length = self.text.chars().count();
        match edit {
            Edit::Insert(value) => {
                let normalized = Policy(self.multiline).prepare(value);
                self.splice(self.selection.range(), &normalized)?;
            }
            Edit::Newline if self.multiline => {
                self.splice(self.selection.range(), "\n")?;
            }
            Edit::Paste(Some(value)) => {
                let cleaned = self.sanitize(value);
                if !cleaned.is_empty() || !self.selection.range().is_empty() {
                    self.splice(self.selection.range(), &cleaned)?;
                }
            }
            Edit::Cut => {
                let range = self.selection.range();
                if !range.is_empty() {
                    self.splice(range, "")?;
                }
            }
            Edit::Backward | Edit::Forward => {
                self.erase(matches!(edit, Edit::Backward), length)?;
            }
            Edit::Move { direction, extend } => {
                self.selection.advance(
                    *direction,
                    *extend,
                    (&self.text, self.multiline, &mut self.column),
                );
            }
            Edit::Select(selection) => self.select(*selection)?,
            Edit::All => {
                self.column = None;
                self.selection = Selection {
                    anchor: 0,
                    focus: length,
                };
            }
            _ => {}
        }
        Ok(old != self.text)
    }

    pub(super) fn cancel(&mut self) -> bool {
        self.composition.take().is_some()
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

    pub(super) fn splice(
        &mut self,
        range: std::ops::Range<usize>,
        value: &str,
    ) -> Result<(), Fault> {
        let bytes = self.byte(range.start)..self.byte(range.end);
        if self.text.len() - bytes.len() + value.len() > self.limit() {
            return Err(Fault("input text budget exhausted".into()));
        }
        self.column = None;
        self.text.replace_range(bytes, value);
        self.selection = Selection::caret(range.start + value.chars().count());
        Ok(())
    }

    pub(super) fn byte(&self, position: usize) -> usize {
        text::byte(&self.text, position)
    }
}
