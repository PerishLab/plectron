use super::compose::{Compose, Outcome};
use super::text::text;
use crate::shared::Keyboard;
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

#[derive(Debug, Default)]
pub(super) struct Keys {
    map: Keymap,
    held: BTreeSet<u8>,
    released: Option<(u8, u32)>,
    compose: Option<Compose>,
}
impl Keys {
    pub(super) fn remap(&mut self, map: Keymap) {
        self.map = map;
        if self.compose.is_none() {
            self.compose = Compose::load();
        }
        self.clear();
    }
    pub(super) fn clear(&mut self) {
        self.held.clear();
        self.released = None;
        self.refocus();
    }
    pub(super) fn refocus(&mut self) {
        if let Some(compose) = self.compose.as_mut() {
            compose.reset();
        }
    }
    pub(super) fn press(&mut self, keycode: u8, state: u16, time: u32) -> Vec<Keyboard> {
        let paired = self
            .released
            .take()
            .is_some_and(|(code, at)| code == keycode && time.wrapping_sub(at) < 2);
        let repeat = !self.held.insert(keycode) || paired;
        let mut inputs = Vec::new();
        if let Some(code) = code(self.map.base(keycode)) {
            inputs.push(Keyboard::Key(Stroke {
                chord: Chord {
                    code,
                    modifiers: modifiers(state),
                },
                repeat,
                text: String::new(),
            }));
        }
        if state & (CONTROL | ALT) == 0
            && let Some(value) = self.text(self.map.symbol(keycode, state))
        {
            inputs.push(Keyboard::Text(value));
        }
        inputs
    }
    fn text(&mut self, keysym: u32) -> Option<String> {
        match self.compose.as_mut().map(|compose| compose.feed(keysym)) {
            Some(Outcome::Hold) => None,
            Some(Outcome::Text(value)) => Some(value),
            _ => text(keysym),
        }
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
        0xff53 => "ArrowRight".into(),
        0xffff => "Delete".into(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Keys {
        let mut keys = Keys::default();
        keys.remap(Keymap::new(
            8,
            2,
            vec![
                0x61, 0x41, 0x31, 0x21, 0xff0d, 0, 0x20, 0, 0xff51, 0, 0xe9, 0xc9, 0xff09, 0xfe20,
            ],
        ));
        keys
    }
    fn stroke(code: &str, modifiers: Modifiers, repeat: bool) -> Keyboard {
        Keyboard::Key(Stroke {
            chord: Chord {
                code: code.into(),
                modifiers,
            },
            repeat,
            text: String::new(),
        })
    }

    #[test]
    fn route() {
        let mut keys = keys();
        let plain = Modifiers::default();
        let shift = Modifiers {
            shift: true,
            ..plain
        };
        assert_eq!(
            keys.press(8, 0, 0),
            vec![stroke("KeyA", plain, false), Keyboard::Text("a".into())]
        );
        assert_eq!(
            keys.press(8, 0, 0),
            vec![stroke("KeyA", plain, true), Keyboard::Text("a".into())]
        );
        keys.release(8, 0);
        assert_eq!(
            keys.press(8, SHIFT, 5),
            vec![stroke("KeyA", shift, false), Keyboard::Text("A".into())]
        );
        keys.release(8, 0);
        assert_eq!(keys.press(8, LOCK, 5)[1], Keyboard::Text("A".into()));
        keys.release(8, 0);
        assert_eq!(
            keys.press(8, LOCK | SHIFT, 5)[1],
            Keyboard::Text("a".into())
        );
        keys.release(8, 0);
        assert_eq!(keys.press(9, LOCK, 0), vec![Keyboard::Text("1".into())]);
        assert_eq!(keys.press(10, 0, 0), vec![stroke("Enter", plain, false)]);
        assert_eq!(
            keys.press(11, 0, 0),
            vec![stroke("Space", plain, false), Keyboard::Text(" ".into())]
        );
        assert_eq!(
            keys.press(12, 0, 0),
            vec![stroke("ArrowLeft", plain, false)]
        );
        assert_eq!(keys.press(13, LOCK, 0), vec![Keyboard::Text("É".into())]);
        assert_eq!(keys.press(14, SHIFT, 0), vec![stroke("Tab", shift, false)]);
        let control = Modifiers {
            control: true,
            ..plain
        };
        keys.clear();
        assert_eq!(
            keys.press(8, CONTROL, 0),
            vec![stroke("KeyA", control, false)]
        );
        let meta = Modifiers {
            meta: true,
            ..plain
        };
        keys.clear();
        assert_eq!(
            keys.press(8, META, 0),
            vec![stroke("KeyA", meta, false), Keyboard::Text("a".into())]
        );
        assert!(keys.press(7, 0, 0).is_empty());
        assert!(keys.press(40, 0, 0).is_empty());
    }

    #[test]
    fn repeat() {
        let mut keys = keys();
        let plain = Modifiers::default();
        let control = Modifiers {
            control: true,
            ..plain
        };
        assert_eq!(keys.press(8, 0, 100)[0], stroke("KeyA", plain, false));
        keys.release(8, 130);
        assert_eq!(
            keys.press(8, 0, 130),
            vec![stroke("KeyA", plain, true), Keyboard::Text("a".into())]
        );
        keys.release(8, 160);
        assert_eq!(keys.press(8, 0, 161)[0], stroke("KeyA", plain, true));
        keys.release(8, 200);
        assert_eq!(keys.press(8, 0, 260)[0], stroke("KeyA", plain, false));
        keys.release(8, 300);
        assert_eq!(keys.press(10, 0, 300), vec![stroke("Enter", plain, false)]);
        assert_eq!(keys.press(8, 0, 300)[0], stroke("KeyA", plain, false));
        keys.release(8, 400);
        keys.release(8, 400);
        assert_eq!(keys.press(8, 0, 400)[0], stroke("KeyA", plain, true));
        keys.release(8, 500);
        keys.clear();
        assert_eq!(keys.press(8, 0, 500)[0], stroke("KeyA", plain, false));
        keys.release(8, u32::MAX);
        assert_eq!(
            keys.press(8, CONTROL, 0),
            vec![stroke("KeyA", control, true)]
        );
    }
}
