#![cfg(test)]

use super::Queue;
use super::compose::{Compose, Outcome};
use super::key::{Keymap, Keys};
use crate::shared::Keyboard;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;

#[test]
fn table() {
    let Some(mut compose) = Compose::load() else {
        eprintln!("skip linux::input::native::table: libxkbcommon.so.0 compose table unavailable");
        return;
    };
    assert_eq!(compose.feed(0xffe1), Outcome::Pass);
    assert_eq!(compose.feed(0xfe51), Outcome::Hold);
    assert_eq!(compose.feed(0x65), Outcome::Text("é".into()));
    assert_eq!(compose.feed(0xff20), Outcome::Hold);
    assert_eq!(compose.feed(0x6f), Outcome::Hold);
    assert_eq!(compose.feed(0x63), Outcome::Text("©".into()));
    assert_eq!(compose.feed(0xfe51), Outcome::Hold);
    assert_eq!(compose.feed(0x71), Outcome::Hold);
    assert_eq!(compose.feed(0x61), Outcome::Pass);
}

#[test]
fn layout() {
    let Ok((connection, _)) = x11rb::connect(None) else {
        eprintln!("skip linux::input::native::layout: no reachable X11 display");
        return;
    };
    let setup = connection.setup();
    let first = setup.min_keycode;
    let count = setup.max_keycode - first + 1;
    let reply = connection
        .get_keyboard_mapping(first, count)
        .unwrap()
        .reply()
        .unwrap();
    let width = usize::from(reply.keysyms_per_keycode);
    let find = |keysym: u32| {
        reply
            .keysyms
            .chunks(width)
            .position(|row| row.first() == Some(&keysym))
            .and_then(|row| u8::try_from(row).ok())
            .map(|row| first + row)
    };
    let codes: Option<Vec<u8>> = [0xfe51, 0x65, 0xff20, 0x6f, 0x63]
        .into_iter()
        .map(find)
        .collect();
    let Some(codes) = codes else {
        eprintln!(
            "skip linux::input::native::layout: layout lacks dead_acute or Multi_key (setxkbmap -layout us -variant intl -option compose:ralt)"
        );
        return;
    };
    if Compose::load().is_none() {
        eprintln!("skip linux::input::native::layout: libxkbcommon.so.0 compose table unavailable");
        return;
    }
    let mut keys = Keys::default();
    keys.remap(Keymap::new(
        first,
        reply.keysyms_per_keycode,
        reply.keysyms.clone(),
    ));
    let mut typed = Vec::new();
    for (time, code) in [0, 1, 2, 3, 4].into_iter().zip(&codes) {
        typed.extend(
            keys.press(*code, 0, time * 10)
                .into_iter()
                .filter_map(|input| match input {
                    Keyboard::Text(value) => Some(value),
                    _ => None,
                }),
        );
        keys.release(*code, time * 10 + 5);
    }
    eprintln!("linux::input::native::layout keycodes={codes:?} text={typed:?}");
    assert_eq!(typed, vec!["é".to_string(), "©".to_string()]);
}

#[test]
fn refocus() {
    if Compose::load().is_none() {
        eprintln!(
            "skip linux::input::native::refocus: libxkbcommon.so.0 compose table unavailable"
        );
        return;
    }
    let queue = Queue::default();
    queue.remap(8, 1, vec![0xfe51, 0x65]);
    let typed = |before: Option<usize>, after: Option<usize>| {
        queue.target(before);
        assert!(queue.keys.borrow_mut().press(8, 0, 0).is_empty());
        queue.target(after);
        let inputs = queue.keys.borrow_mut().press(9, 0, 1);
        inputs.into_iter().find_map(|input| match input {
            Keyboard::Text(value) => Some(value),
            _ => None,
        })
    };
    let text = |value: &str| Some(value.to_string());
    assert_eq!(typed(Some(1), Some(1)), text("é"));
    assert_eq!(typed(Some(1), Some(2)), text("e"));
    assert_eq!(typed(Some(2), None), text("e"));
    assert_eq!(typed(None, Some(3)), text("e"));
}
