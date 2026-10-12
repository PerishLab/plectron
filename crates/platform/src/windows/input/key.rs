use crate::shared::Keyboard;
use shared::shortcut::{Chord, Modifiers, Stroke};
use windows_sys::Win32::{
    Foundation::{LPARAM, WPARAM},
    UI::Input::KeyboardAndMouse::GetKeyState,
};

pub(super) fn stroke(data: (WPARAM, LPARAM)) -> Option<Keyboard> {
    let code = match data.0 {
        0x41..=0x5a => format!("Key{}", char::from_u32(data.0 as u32)?),
        0x08 => "Backspace".into(),
        0x09 => "Tab".into(),
        0x0d => "Enter".into(),
        0x1b => "Escape".into(),
        0x20 => "Space".into(),
        0x23 => "End".into(),
        0x24 => "Home".into(),
        0x25 => "ArrowLeft".into(),
        0x27 => "ArrowRight".into(),
        0x2e => "Delete".into(),
        _ => return None,
    };
    Some(Keyboard::Key(Stroke {
        chord: Chord {
            code,
            modifiers: Modifiers {
                shift: pressed(0x10),
                control: pressed(0x11),
                alt: pressed(0x12),
                meta: pressed(0x5b) || pressed(0x5c),
            },
        },
        repeat: data.1 & (1 << 30) != 0,
        text: String::new(),
    }))
}
fn pressed(code: i32) -> bool {
    // SAFETY: GetKeyState reads the current owner thread's translated keyboard state without borrowed pointers.
    unsafe { GetKeyState(code) < 0 }
}
