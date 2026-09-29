#![cfg(target_os = "macos")]

use platform::shared::{Composition, Event, Id, Selection, Signal, Unit};
use shared::shortcut::{Chord, Input, Stroke};

#[test]
fn editing() {
    let unit = Unit::new(1);
    let mut input = platform::macos::Keyboard::default();
    let mut callback = input.clone();
    callback.key(123, false, 0);
    callback.key(36, false, 0);
    let mut signals = Vec::new();
    input.drain(unit, &mut signals).unwrap();
    assert_eq!(
        events(signals),
        vec![key("ArrowLeft", false), key("Enter", false)]
    );
    callback.key(123, false, 1 << 17);
    callback.key(0, false, 1 << 20);
    input.release();
    let mut signals = Vec::new();
    input.drain(unit, &mut signals).unwrap();
    assert_eq!(
        events(signals),
        vec![key("Shift+ArrowLeft", false), key("Primary+KeyA", false)]
    );
    callback.key(0, false, 0);
    callback.key(0, true, 0);
    let mut signals = Vec::new();
    input.drain(unit, &mut signals).unwrap();
    assert_eq!(events(signals), vec![key("KeyA", false), key("KeyA", true)]);
}

#[test]
fn composition() {
    use platform::macos::Text;

    assert_eq!(
        Text::Mark {
            id: 7,
            text: "A😀中".into(),
            start: 1,
            end: 3,
        }
        .event()
        .unwrap(),
        Event::Composition(Composition::Mark {
            id: Id(7),
            text: "A😀中".into(),
            selection: Selection {
                anchor: 1,
                focus: 2,
            },
        })
    );
    assert_eq!(
        Text::Commit {
            id: 7,
            text: "钟".into(),
        }
        .event()
        .unwrap(),
        Event::Composition(Composition::Commit {
            id: Id(7),
            text: "钟".into(),
        })
    );
    assert_eq!(
        Text::Cancel { id: 7 }.event().unwrap(),
        Event::Composition(Composition::Cancel { id: Id(7) })
    );
    assert_eq!(
        Text::Insert("中文".into()).event().unwrap(),
        Event::Input(Input::Text("中文".into()))
    );
    assert!(
        Text::Mark {
            id: 7,
            text: "😀".into(),
            start: 1,
            end: 2,
        }
        .event()
        .is_err()
    );
}

fn events(signals: Vec<Signal>) -> Vec<Event> {
    signals.into_iter().map(|signal| signal.event).collect()
}

fn key(chord: &str, repeat: bool) -> Event {
    Event::Input(Input::Key(Stroke {
        chord: Chord::parse(chord).unwrap(),
        text: String::new(),
        repeat,
    }))
}
