use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Left,
    Right,
    Start,
    End,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub anchor: usize,
    pub focus: usize,
}

impl Selection {
    pub fn caret(position: usize) -> Self {
        Self {
            anchor: position,
            focus: position,
        }
    }

    pub fn range(self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    pub(super) fn advance(&mut self, direction: Direction, extend: bool, length: usize) {
        let selected = self.anchor != self.focus && !extend;
        self.focus = match direction {
            Direction::Left if selected => self.range().start,
            Direction::Right if selected => self.range().end,
            Direction::Left => self.focus.saturating_sub(1),
            Direction::Right => (self.focus + 1).min(length),
            Direction::Start => 0,
            Direction::End => length,
        };
        if !extend {
            self.anchor = self.focus;
        }
    }
}
