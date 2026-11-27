use serde::{Deserialize, Serialize};
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
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

    pub(super) fn advance(
        &mut self,
        direction: Direction,
        extend: bool,
        context: (&str, bool, &mut Option<usize>),
    ) {
        let (text, multiline, column) = context;
        let length = text.chars().count();
        let selected = self.anchor != self.focus && !extend;
        if matches!(direction, Direction::Up | Direction::Down) {
            self.vertical(direction == Direction::Down, extend, (text, column));
            return;
        }
        *column = None;
        self.focus = match direction {
            Direction::Left if selected => self.range().start,
            Direction::Right if selected => self.range().end,
            Direction::Left => self.focus.saturating_sub(1),
            Direction::Right => (self.focus + 1).min(length),
            Direction::Start if multiline => bounds(text, self.focus).start,
            Direction::End if multiline => bounds(text, self.focus).end,
            Direction::Start => 0,
            Direction::End => length,
            Direction::Up | Direction::Down => unreachable!(),
        };
        if !extend {
            self.anchor = self.focus;
        }
    }

    fn vertical(&mut self, down: bool, extend: bool, context: (&str, &mut Option<usize>)) {
        let (text, column) = context;
        let spans = lines(text);
        let origin = if self.anchor != self.focus && !extend {
            if down {
                self.range().end
            } else {
                self.range().start
            }
        } else {
            self.focus
        };
        let current = locate(&spans, origin);
        let offset = origin - spans[current].start;
        let preferred = *column.get_or_insert(offset);
        let target = if down {
            (current + 1 < spans.len()).then_some(current + 1)
        } else {
            current.checked_sub(1)
        };
        if let Some(next) = target {
            let span = &spans[next];
            self.focus = span.start + preferred.min(span.end - span.start);
        } else {
            self.focus = origin;
        }
        if !extend {
            self.anchor = self.focus;
        }
    }
}

fn lines(text: &str) -> Vec<Range<usize>> {
    let mut spans = Vec::new();
    let mut start = 0;
    let mut count = 0;
    for character in text.chars() {
        if character == '\n' {
            spans.push(start..count);
            start = count + 1;
        }
        count += 1;
    }
    spans.push(start..count);
    spans
}

fn locate(spans: &[Range<usize>], position: usize) -> usize {
    spans
        .partition_point(|span| span.start <= position)
        .saturating_sub(1)
}

fn bounds(text: &str, position: usize) -> Range<usize> {
    let spans = lines(text);
    spans[locate(&spans, position)].clone()
}
