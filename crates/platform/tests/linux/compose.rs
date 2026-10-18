#![cfg(test)]

use super::Queue;
use super::compose::{Compose, Outcome};
use super::key::{Keymap, Keys};
use crate::shared::{Composition, Event, Id, Keyboard, Selection};
use shared::shortcut::{Chord, Modifiers, Stroke};
use x11rb::connection::Connection;
use x11rb::protocol::Event as XEvent;
use x11rb::protocol::xproto::{ConnectionExt, FocusInEvent, NotifyDetail, NotifyMode};

fn mark(id: u64, text: &str) -> Event {
    Event::Composition(Composition::Mark {
        id: Id(id),
        text: text.into(),
        selection: Selection::caret(text.chars().count()),
    })
}
fn commit(id: u64, text: &str) -> Event {
    Event::Composition(Composition::Commit {
        id: Id(id),
        text: text.into(),
    })
}
fn cancel(id: u64) -> Event {
    Event::Composition(Composition::Cancel { id: Id(id) })
}
fn key(code: &str, control: bool) -> Event {
    Event::Input(Keyboard::Key(Stroke {
        chord: Chord {
            code: code.into(),
            modifiers: Modifiers {
                control,
                ..Modifiers::default()
            },
        },
        repeat: false,
        text: String::new(),
    }))
}
fn focus(inside: bool) -> XEvent {
    let event = FocusInEvent {
        response_type: if inside { 9 } else { 10 },
        detail: NotifyDetail::NONLINEAR,
        sequence: 0,
        event: 2,
        mode: NotifyMode::NORMAL,
    };
    if inside {
        XEvent::FocusIn(event)
    } else {
        XEvent::FocusOut(event)
    }
}

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
    assert_eq!(compose.feed(0x71), Outcome::Cancel);
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
                .filter_map(|event| match event {
                    Event::Composition(Composition::Commit { text, .. }) => Some(text),
                    Event::Input(Keyboard::Text(value)) => Some(value),
                    _ => None,
                }),
        );
        keys.release(*code, time * 10 + 5);
    }
    eprintln!("linux::input::native::layout keycodes={codes:?} text={typed:?}");
    assert_eq!(typed, vec!["é".to_string(), "©".to_string()]);
}

#[test]
fn session() {
    if Compose::load().is_none() {
        eprintln!(
            "skip linux::input::native::session: libxkbcommon.so.0 compose table unavailable"
        );
        return;
    }
    let mut keys = Keys::default();
    assert_eq!(
        keys.remap(Keymap::new(
            8,
            1,
            vec![0xfe51, 0x65, 0xff20, 0x6f, 0x63, 0x71]
        )),
        None
    );
    let mut time = 0;
    let mut tap = |keys: &mut Keys, code: u8, state: u16| {
        time += 10;
        let events = keys.press(code, state, time);
        keys.release(code, time + 1);
        events
    };
    assert_eq!(tap(&mut keys, 8, 0), vec![mark(1, "\u{b4}")]);
    assert_eq!(tap(&mut keys, 9, 0), vec![commit(1, "\u{e9}")]);
    assert_eq!(tap(&mut keys, 10, 0), vec![mark(2, "\u{b7}")]);
    assert_eq!(tap(&mut keys, 11, 0), vec![mark(2, "\u{b7}o")]);
    assert_eq!(tap(&mut keys, 12, 0), vec![commit(2, "\u{a9}")]);
    assert_eq!(tap(&mut keys, 8, 0), vec![mark(3, "\u{b4}")]);
    assert_eq!(tap(&mut keys, 13, 0), vec![cancel(3)]);
    assert_eq!(
        tap(&mut keys, 9, 0),
        vec![key("KeyE", false), Event::Input(Keyboard::Text("e".into()))]
    );
    assert_eq!(tap(&mut keys, 8, 0), vec![mark(4, "\u{b4}")]);
    assert_eq!(tap(&mut keys, 9, 4), vec![cancel(4), key("KeyE", true)]);
    assert_eq!(tap(&mut keys, 8, 0), vec![mark(5, "\u{b4}")]);
    assert_eq!(keys.clear(), Some(cancel(5)));
    assert_eq!(keys.refocus(), None);
    assert_eq!(tap(&mut keys, 8, 0), vec![mark(6, "\u{b4}")]);
    assert_eq!(
        keys.remap(Keymap::new(8, 1, vec![0xfe51, 0x65])),
        Some(cancel(6))
    );
    assert_eq!(
        tap(&mut keys, 9, 0),
        vec![key("KeyE", false), Event::Input(Keyboard::Text("e".into()))]
    );
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
    queue.remap(8, 1, vec![0xfe51, 0x65]).unwrap();
    let extent = (64, 32);
    queue.dispatch(&focus(true), extent).unwrap();
    let typed = |before: Option<usize>, after: Option<usize>| {
        queue.target(before).unwrap();
        let start = queue.keys.borrow_mut().press(8, 0, 0);
        queue.keys.borrow_mut().release(8, 0);
        queue.target(after).unwrap();
        let events = queue.drain();
        let next = queue.keys.borrow_mut().press(9, 0, 1);
        queue.keys.borrow_mut().release(9, 1);
        [start, events, next].concat()
    };
    let text = Event::Input(Keyboard::Text("e".into()));
    assert_eq!(
        typed(Some(1), Some(1)),
        vec![mark(1, "\u{b4}"), commit(1, "\u{e9}")]
    );
    assert_eq!(
        typed(Some(1), Some(2)),
        vec![
            mark(2, "\u{b4}"),
            cancel(2),
            key("KeyE", false),
            text.clone()
        ]
    );
    assert_eq!(
        typed(Some(2), None),
        vec![
            mark(3, "\u{b4}"),
            cancel(3),
            key("KeyE", false),
            text.clone()
        ]
    );
    assert_eq!(
        typed(None, Some(3)),
        vec![mark(4, "\u{b4}"), cancel(4), key("KeyE", false), text]
    );
    queue.keys.borrow_mut().press(8, 0, 2);
    queue.dispatch(&focus(false), extent).unwrap();
    assert_eq!(queue.drain(), vec![cancel(5)]);
}
