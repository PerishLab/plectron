use super::{Queue, ime, imm};
use crate::shared::{Caret, Composition, Event, Focus, Id, Keyboard, Selection};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WM_CHAR, WM_IME_CHAR, WM_IME_ENDCOMPOSITION, WM_IME_STARTCOMPOSITION, WM_SETFOCUS,
};

#[test]
fn bound() {
    let queue = Queue::default();
    for _ in 0..512 {
        queue.push(Event::Scroll { delta: 1 }).unwrap();
    }
    assert!(queue.push(Event::Scroll { delta: 1 }).is_err());
    assert_eq!(queue.drain().len(), 16);
    assert!(queue.pending());
    queue.close();
    queue.push(Event::Scroll { delta: 1 }).unwrap();
    assert!(!queue.pending());
}

#[test]
fn composition() {
    let mut state = ime::State::default();
    let mut events = Vec::new();
    state.start();
    let marked: Vec<u16> = "中🙂文".encode_utf16().collect();
    state
        .compose(Some((&marked, Some(3))), None, &mut events)
        .unwrap();
    assert!(state.suppress('a' as usize));
    let committed: Vec<u16> = "你好🙂".encode_utf16().collect();
    let next: Vec<u16> = "a".encode_utf16().collect();
    state
        .compose(Some((&next, Some(1))), Some(&committed), &mut events)
        .unwrap();
    state.end(&mut events);
    assert_eq!(
        events,
        vec![
            Event::Composition(Composition::Mark {
                id: Id(1),
                text: "中🙂文".into(),
                selection: Selection::caret(2),
            }),
            Event::Composition(Composition::Commit {
                id: Id(1),
                text: "你好🙂".into(),
            }),
            Event::Composition(Composition::Mark {
                id: Id(2),
                text: "a".into(),
                selection: Selection::caret(1),
            }),
            Event::Composition(Composition::Cancel { id: Id(2) }),
        ]
    );
}

#[test]
fn duplicate() {
    let queue = Queue::default();
    let null = std::ptr::null_mut();
    queue.dispatch(null, WM_SETFOCUS, (0, 0)).unwrap();
    queue
        .dispatch(null, WM_IME_STARTCOMPOSITION, (0, 0))
        .unwrap();
    queue.dispatch(null, WM_CHAR, ('x' as usize, 0)).unwrap();
    queue.dispatch(null, WM_IME_CHAR, (0x4e2d, 0)).unwrap();
    let committed: Vec<u16> = "中🙂".encode_utf16().collect();
    let mut events = Vec::new();
    queue
        .ime
        .borrow_mut()
        .compose(None, Some(&committed), &mut events)
        .unwrap();
    queue.emit(events).unwrap();
    queue.dispatch(null, WM_IME_ENDCOMPOSITION, (0, 0)).unwrap();
    for unit in committed {
        queue.dispatch(null, WM_CHAR, (unit as usize, 0)).unwrap();
    }
    queue.dispatch(null, WM_CHAR, ('Z' as usize, 0)).unwrap();
    assert_eq!(
        queue.drain(),
        vec![
            Event::Composition(Composition::Mark {
                id: Id(1),
                text: String::new(),
                selection: Selection::caret(0),
            }),
            Event::Composition(Composition::Commit {
                id: Id(1),
                text: "中🙂".into(),
            }),
            Event::Input(Keyboard::Text("Z".into())),
        ]
    );
}

#[test]
fn focus() {
    let mut state = ime::State::default();
    let mut events = Vec::new();
    let first = Focus {
        target: 1,
        text: String::new(),
        selection: Selection::caret(0),
        caret: Caret {
            x: 10,
            y: 20,
            width: 2,
            height: 16,
        },
    };
    let second = Focus {
        target: 2,
        ..first.clone()
    };
    assert_eq!(
        state.focus(Some(&first), 144, &mut events),
        (false, Some(first.caret))
    );
    assert_eq!(state.focus(Some(&first), 144, &mut events), (false, None));
    state.start();
    let units: Vec<u16> = "ni".encode_utf16().collect();
    state
        .compose(Some((&units, Some(2))), None, &mut events)
        .unwrap();
    events.clear();
    assert_eq!(
        state.focus(Some(&second), 144, &mut events),
        (true, Some(second.caret))
    );
    assert_eq!(
        events,
        vec![Event::Composition(Composition::Cancel { id: Id(1) })]
    );
    events.clear();
    state.start();
    state
        .compose(Some((&units, Some(1))), None, &mut events)
        .unwrap();
    events.clear();
    assert_eq!(state.focus(None, 144, &mut events), (true, None));
    assert_eq!(
        events,
        vec![Event::Composition(Composition::Cancel { id: Id(2) })]
    );
    let (form, candidate) = imm::geometry(first.caret, 144);
    assert_eq!((form.point.x, form.point.y), (15, 30));
    assert_eq!((candidate.point.x, candidate.point.y), (15, 54));
    assert_eq!((candidate.area.right, candidate.area.bottom), (18, 54));
    assert!(imm::ready());
}
