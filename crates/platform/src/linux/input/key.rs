use super::compose::{Compose, Outcome};
use super::text::{glyph, text};
use crate::shared::{Composition, Event, Id, Keyboard, Selection};
use shared::shortcut::{Chord, Modifiers, Stroke};
use std::collections::BTreeSet;

const SHIFT: u16 = 1;
const LOCK: u16 = 1 << 1;
const CONTROL: u16 = 1 << 2;
const ALT: u16 = 1 << 3;
const META: u16 = 1 << 6;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct Keymap {
    first: u8,
    width: usize,
    keysyms: Vec<u32>,
}
impl Keymap {
    pub(super) fn new(first: u8, width: u8, keysyms: Vec<u32>) -> Self {
        Self {
            first,
            width: usize::from(width),
            keysyms,
        }
    }
    fn column(&self, keycode: u8, column: usize) -> u32 {
        let Some(row) = keycode.checked_sub(self.first) else {
            return 0;
        };
        if column >= self.width {
            return 0;
        }
        self.keysyms
            .get(usize::from(row) * self.width + column)
            .copied()
            .unwrap_or(0)
    }
    fn base(&self, keycode: u8) -> u32 {
        self.column(keycode, 0)
    }
    fn symbol(&self, keycode: u8, state: u16) -> u32 {
        let lower = self.column(keycode, 0);
        let upper = match self.column(keycode, 1) {
            0 => upper(lower),
            value => value,
        };
        let shifted = state & SHIFT != 0;
        let locked = state & LOCK != 0 && upper != lower && letter(lower);
        if shifted != locked { upper } else { lower }
    }
}

#[derive(Debug)]
struct Session {
    id: Id,
    text: String,
}

#[derive(Debug, Default)]
pub(super) struct Keys {
    map: Keymap,
    held: BTreeSet<u8>,
    released: Option<(u8, u32)>,
    compose: Option<Compose>,
    session: Option<Session>,
    serial: u64,
}
impl Keys {
    pub(super) fn remap(&mut self, map: Keymap) -> Option<Event> {
        self.map = map;
        if self.compose.is_none() {
            self.compose = Compose::load();
        }
        self.clear()
    }
    pub(super) fn clear(&mut self) -> Option<Event> {
        self.held.clear();
        self.released = None;
        self.refocus()
    }
    pub(super) fn refocus(&mut self) -> Option<Event> {
        if let Some(compose) = self.compose.as_mut() {
            compose.reset();
        }
        let session = self.session.take()?;
        Some(Event::Composition(Composition::Cancel { id: session.id }))
    }
    pub(super) fn press(&mut self, keycode: u8, state: u16, time: u32) -> Vec<Event> {
        let paired = self
            .released
            .take()
            .is_some_and(|(code, at)| code == keycode && time.wrapping_sub(at) < 2);
        let repeat = !self.held.insert(keycode) || paired;
        let key = code(self.map.base(keycode)).map(|code| {
            Event::Input(Keyboard::Key(Stroke {
                chord: Chord {
                    code,
                    modifiers: modifiers(state),
                },
                repeat,
                text: String::new(),
            }))
        });
        if state & (CONTROL | ALT) != 0 {
            return self.refocus().into_iter().chain(key).collect();
        }
        let keysym = self.map.symbol(keycode, state);
        let outcome = self
            .compose
            .as_mut()
            .map_or(Outcome::Pass, |compose| compose.feed(keysym));
        match (self.session.take(), outcome) {
            (None, Outcome::Hold) => {
                self.serial += 1;
                let session = Session {
                    id: Id(self.serial),
                    text: glyph(keysym),
                };
                key.into_iter().chain([self.mark(session)]).collect()
            }
            (Some(mut session), Outcome::Hold) => {
                session.text.push_str(&glyph(keysym));
                vec![self.mark(session)]
            }
            (Some(session), Outcome::Text(text)) => {
                vec![Event::Composition(Composition::Commit {
                    id: session.id,
                    text,
                })]
            }
            (Some(session), Outcome::Cancel) => {
                vec![Event::Composition(Composition::Cancel { id: session.id })]
            }
            (session, outcome) => {
                self.session = session;
                let value = match outcome {
                    Outcome::Text(value) => Some(value),
                    _ => text(keysym),
                };
                let value = value.map(|value| Event::Input(Keyboard::Text(value)));
                key.into_iter().chain(value).collect()
            }
        }
    }
    fn mark(&mut self, session: Session) -> Event {
        let event = Event::Composition(Composition::Mark {
            id: session.id,
            text: session.text.clone(),
            selection: Selection::caret(session.text.chars().count()),
        });
        self.session = Some(session);
        event
    }
    pub(super) fn release(&mut self, keycode: u8, time: u32) {
        if self.held.remove(&keycode) {
            self.released = Some((keycode, time));
        }
    }
}

fn modifiers(state: u16) -> Modifiers {
    Modifiers {
        shift: state & SHIFT != 0,
        control: state & CONTROL != 0,
        alt: state & ALT != 0,
        meta: state & META != 0,
    }
}
fn letter(keysym: u32) -> bool {
    (0x61..=0x7a).contains(&keysym) || (0xe0..=0xfe).contains(&keysym) && keysym != 0xf7
}
fn upper(keysym: u32) -> u32 {
    if letter(keysym) {
        keysym - 0x20
    } else {
        keysym
    }
}
fn code(keysym: u32) -> Option<String> {
    Some(match keysym {
        0x61..=0x7a => format!("Key{}", char::from_u32(keysym - 0x20)?),
        0x41..=0x5a => format!("Key{}", char::from_u32(keysym)?),
        0xff08 => "Backspace".into(),
        0xff09 | 0xfe20 => "Tab".into(),
        0xff0d | 0xff8d => "Enter".into(),
        0xff1b => "Escape".into(),
        0x20 => "Space".into(),
        0xff57 => "End".into(),
        0xff50 => "Home".into(),
        0xff51 => "ArrowLeft".into(),
        0xff52 => "ArrowUp".into(),
        0xff53 => "ArrowRight".into(),
        0xff54 => "ArrowDown".into(),
        0xffff => "Delete".into(),
        _ => return None,
    })
}

#[cfg(test)]
#[path = "../../../tests/linux/key.rs"]
mod tests;
