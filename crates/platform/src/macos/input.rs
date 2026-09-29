use super::Error;
use crate::shared::{Event, Signal, Unit};
use ::shared::shortcut::{Chord, Input as Packet, Modifiers, Stroke};
use std::{cell::RefCell, rc::Rc};

#[derive(Clone, Default)]
pub struct Keyboard(Rc<RefCell<Pending>>);

#[derive(Default)]
struct Pending {
    events: Vec<Packet>,
    overflow: bool,
}

impl Keyboard {
    pub fn key(&mut self, native: u16, repeat: bool, flags: u64) {
        let Some(code) = code(native) else {
            return;
        };
        let mut pending = self.0.borrow_mut();
        if pending.events.len() >= 256 {
            pending.overflow = true;
            return;
        }
        pending.events.push(Packet::Key(Stroke {
            chord: Chord {
                code,
                modifiers: modifiers(flags),
            },
            repeat,
            text: String::new(),
        }));
    }

    pub fn release(&mut self) {}

    pub fn drain(&mut self, unit: Unit, signals: &mut Vec<Signal>) -> Result<(), Error> {
        let mut pending = self.0.borrow_mut();
        if pending.overflow {
            *pending = Pending::default();
            return Err(Error("native keyboard input budget exhausted".into()));
        }
        signals.extend(pending.events.drain(..).map(|input| Signal {
            unit,
            event: Event::Input(input),
        }));
        Ok(())
    }
}

fn modifiers(flags: u64) -> Modifiers {
    Modifiers {
        shift: flags & (1 << 17) != 0,
        control: flags & (1 << 18) != 0,
        alt: flags & (1 << 19) != 0,
        meta: flags & (1 << 20) != 0,
    }
}

fn code(key: u16) -> Option<String> {
    let code = match key {
        0 => "KeyA",
        1 => "KeyS",
        2 => "KeyD",
        3 => "KeyF",
        4 => "KeyH",
        5 => "KeyG",
        6 => "KeyZ",
        7 => "KeyX",
        8 => "KeyC",
        9 => "KeyV",
        11 => "KeyB",
        12 => "KeyQ",
        13 => "KeyW",
        14 => "KeyE",
        15 => "KeyR",
        16 => "KeyY",
        17 => "KeyT",
        31 => "KeyO",
        32 => "KeyU",
        34 => "KeyI",
        35 => "KeyP",
        36 | 76 => "Enter",
        37 => "KeyL",
        38 => "KeyJ",
        40 => "KeyK",
        45 => "KeyN",
        46 => "KeyM",
        48 => "Tab",
        49 => "Space",
        51 => "Backspace",
        53 => "Escape",
        115 => "Home",
        117 => "Delete",
        119 => "End",
        123 => "ArrowLeft",
        124 => "ArrowRight",
        125 => "ArrowDown",
        126 => "ArrowUp",
        _ => return None,
    };
    Some(code.into())
}
