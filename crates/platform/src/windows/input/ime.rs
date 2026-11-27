use super::{Error, imm};
use crate::shared::{Caret, Composition, Event, Focus, Id, Selection};
use std::collections::VecDeque;
use windows_sys::Win32::Foundation::HWND;

#[derive(Default)]
pub(super) struct State {
    sequence: u64,
    active: Option<u64>,
    marked: bool,
    target: Option<usize>,
    placed: Option<(usize, u32, Caret)>,
    suppress: VecDeque<u16>,
}

impl State {
    pub(super) fn active(&self) -> bool {
        self.active.is_some()
    }

    pub(super) fn clear(&mut self) {
        self.suppress.clear();
    }

    pub(super) fn start(&mut self) {
        if self.active.is_none() {
            self.active = Some(self.next());
            self.marked = false;
        }
        self.suppress.clear();
    }

    pub(super) fn read(
        &mut self,
        handle: HWND,
        flags: u32,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let Some(guard) = imm::Guard::open(handle) else {
            return Ok(());
        };
        let result = (flags & imm::RESULT != 0)
            .then(|| guard.string(imm::RESULT))
            .flatten();
        let marked = (flags & imm::COMPOSITION != 0)
            .then(|| {
                let units = guard.string(imm::COMPOSITION)?;
                let cursor = (flags & imm::CURSOR != 0).then(|| guard.cursor()).flatten();
                Some((units, cursor))
            })
            .flatten();
        self.compose(
            marked
                .as_ref()
                .map(|(units, cursor)| (units.as_slice(), *cursor)),
            result.as_deref(),
            events,
        )
    }

    pub(super) fn compose(
        &mut self,
        marked: Option<(&[u16], Option<usize>)>,
        result: Option<&[u16]>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        if let Some(units) = result {
            self.commit(units, events)?;
        }
        if let Some((units, cursor)) = marked {
            if result.is_some() && units.is_empty() {
                return Ok(());
            }
            self.mark(units, cursor, events)?;
        }
        Ok(())
    }

    pub(super) fn end(&mut self, events: &mut Vec<Event>) {
        if let Some(id) = self.active.take()
            && self.marked
        {
            events.push(Event::Composition(Composition::Cancel { id: Id(id) }));
        }
        self.marked = false;
    }

    pub(super) fn cancel(&mut self, events: &mut Vec<Event>) -> bool {
        let active = self.active.take();
        if let Some(id) = active
            && self.marked
        {
            events.push(Event::Composition(Composition::Cancel { id: Id(id) }));
        }
        self.marked = false;
        self.suppress.clear();
        active.is_some()
    }

    pub(super) fn focus(
        &mut self,
        context: Option<&Focus>,
        dpi: u32,
        events: &mut Vec<Event>,
    ) -> (bool, Option<Caret>) {
        let target = context.map(|focus| focus.target);
        let switched = target != self.target;
        self.target = target;
        let cancelled = if switched {
            self.placed = None;
            self.cancel(events)
        } else {
            false
        };
        let Some(focus) = context else {
            return (cancelled, None);
        };
        let key = (focus.target, dpi, focus.caret);
        if self.placed == Some(key) {
            return (cancelled, None);
        }
        self.placed = Some(key);
        (cancelled, Some(focus.caret))
    }

    pub(super) fn suppress(&mut self, value: usize) -> bool {
        if self.active.is_some() {
            return true;
        }
        let Ok(unit) = u16::try_from(value) else {
            self.suppress.clear();
            return false;
        };
        if self.suppress.front() == Some(&unit) {
            self.suppress.pop_front();
            return true;
        }
        self.suppress.clear();
        false
    }

    fn next(&mut self) -> u64 {
        self.sequence += 1;
        self.sequence
    }

    fn mark(
        &mut self,
        units: &[u16],
        cursor: Option<usize>,
        events: &mut Vec<Event>,
    ) -> Result<(), Error> {
        let text = String::from_utf16(units)
            .map_err(|_| Error("invalid native IME UTF-16 composition".into()))?;
        let id = match self.active {
            Some(id) => id,
            None => {
                let id = self.next();
                self.active = Some(id);
                id
            }
        };
        let caret = cursor.map_or_else(|| text.chars().count(), |offset| scalar(&text, offset));
        self.marked = true;
        events.push(Event::Composition(Composition::Mark {
            id: Id(id),
            text,
            selection: Selection::caret(caret),
        }));
        Ok(())
    }

    fn commit(&mut self, units: &[u16], events: &mut Vec<Event>) -> Result<(), Error> {
        let text = String::from_utf16(units)
            .map_err(|_| Error("invalid native IME UTF-16 result".into()))?;
        self.suppress.clear();
        self.suppress.extend(units.iter().copied());
        let id = self.active.take();
        let marked = std::mem::replace(&mut self.marked, false);
        if text.is_empty() && !marked {
            return Ok(());
        }
        let id = id.unwrap_or_else(|| self.next());
        if !marked {
            events.push(Event::Composition(Composition::Mark {
                id: Id(id),
                text: String::new(),
                selection: Selection::caret(0),
            }));
        }
        events.push(Event::Composition(Composition::Commit { id: Id(id), text }));
        Ok(())
    }
}

fn scalar(text: &str, position: usize) -> usize {
    let mut units = 0;
    for (index, character) in text.chars().enumerate() {
        if units >= position {
            return index;
        }
        units += character.len_utf16();
    }
    text.chars().count()
}
