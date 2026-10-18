use super::*;

fn keys() -> Keys {
    let mut keys = Keys::default();
    keys.remap(Keymap::new(
        8,
        2,
        vec![
            0x61, 0x41, 0x31, 0x21, 0xff0d, 0, 0x20, 0, 0xff51, 0, 0xe9, 0xc9, 0xff09, 0xfe20,
            0xff52, 0, 0xff54, 0,
        ],
    ));
    keys
}
fn stroke(code: &str, modifiers: Modifiers, repeat: bool) -> Event {
    Event::Input(Keyboard::Key(Stroke {
        chord: Chord {
            code: code.into(),
            modifiers,
        },
        repeat,
        text: String::new(),
    }))
}

fn typed(value: &str) -> Event {
    Event::Input(Keyboard::Text(value.into()))
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
        vec![stroke("KeyA", plain, false), typed("a")]
    );
    assert_eq!(
        keys.press(8, 0, 0),
        vec![stroke("KeyA", plain, true), typed("a")]
    );
    keys.release(8, 0);
    assert_eq!(
        keys.press(8, SHIFT, 5),
        vec![stroke("KeyA", shift, false), typed("A")]
    );
    keys.release(8, 0);
    assert_eq!(keys.press(8, LOCK, 5)[1], typed("A"));
    keys.release(8, 0);
    assert_eq!(keys.press(8, LOCK | SHIFT, 5)[1], typed("a"));
    keys.release(8, 0);
    assert_eq!(keys.press(9, LOCK, 0), vec![typed("1")]);
    assert_eq!(keys.press(10, 0, 0), vec![stroke("Enter", plain, false)]);
    assert_eq!(
        keys.press(11, 0, 0),
        vec![stroke("Space", plain, false), typed(" ")]
    );
    assert_eq!(
        keys.press(12, 0, 0),
        vec![stroke("ArrowLeft", plain, false)]
    );
    assert_eq!(keys.press(13, LOCK, 0), vec![typed("É")]);
    assert_eq!(keys.press(14, SHIFT, 0), vec![stroke("Tab", shift, false)]);
    let vertical = [keys.press(15, 0, 0), keys.press(16, SHIFT, 0)].concat();
    let expected = [
        stroke("ArrowUp", plain, false),
        stroke("ArrowDown", shift, false),
    ];
    assert_eq!(vertical, expected);
    assert!(vertical.iter().all(|event| match event {
        Event::Input(input) => input.validate().is_ok(),
        _ => false,
    }));
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
        vec![stroke("KeyA", meta, false), typed("a")]
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
        vec![stroke("KeyA", plain, true), typed("a")]
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
