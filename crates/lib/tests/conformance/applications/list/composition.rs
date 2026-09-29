use super::{evaluate, setup};
use crate::web::{call, snapshot};
use platform::shared::Selection as NativeSelection;
use plectron::ipc::Command;
use plectron::platform::device::{Composition as Native, Event, Id as NativeId, Signal};
use webengine::Selection;
use webengine::ime::{Composition, Id, Input};

#[test]
fn boundary() {
    let (mut session, _) = setup();
    evaluate(
        &mut session,
        "title.setValue('A中'); title.setSelection(1, 2); title.focus();",
    );
    let id = Id(23);
    call(
        &mut session,
        Command::Composition {
            unit: 1,
            input: Input::Update(Composition {
                id,
                replacement: Selection {
                    anchor: 1,
                    focus: 2,
                },
                text: "zhong".into(),
                selection: Selection::caret(5),
                spans: Vec::new(),
            }),
        },
    );
    session.tick().unwrap();
    let current = snapshot(&mut session);
    let editor = current
        .nodes
        .iter()
        .find(|node| {
            node.attributes
                .get("id")
                .is_some_and(|value| value == "title")
        })
        .unwrap()
        .editor
        .as_ref()
        .unwrap();
    assert_eq!(editor.text, "A中");
    assert_eq!(editor.composition.as_ref().unwrap().text, "zhong");
    call(
        &mut session,
        Command::Composition {
            unit: 1,
            input: Input::Commit {
                id,
                text: "钟".into(),
            },
        },
    );
    session.tick().unwrap();
    assert_eq!(evaluate(&mut session, "title.getValue();"), "A钟");
    call(&mut session, Command::Shutdown {});
}

#[test]
fn virtualized() {
    let (mut session, device) = setup();
    evaluate(
        &mut session,
        "title.setValue('A中'); title.setSelection(1, 2); title.focus();",
    );
    let id = NativeId(24);
    let send = |device: &super::Device, input| {
        let unit = *device.0.borrow().windows.keys().next().unwrap();
        device.0.borrow_mut().signals.push(Signal {
            unit,
            event: Event::Composition(input),
        });
    };
    send(
        &device,
        Native::Mark {
            id,
            text: "zhong".into(),
            selection: NativeSelection::caret(5),
        },
    );
    session.tick().unwrap();
    let page = snapshot(&mut session);
    let editor = page
        .nodes
        .iter()
        .find(|node| {
            node.attributes
                .get("id")
                .is_some_and(|value| value == "title")
        })
        .unwrap()
        .editor
        .as_ref()
        .unwrap();
    assert_eq!(editor.text, "A中");
    assert_eq!(editor.composition.as_ref().unwrap().text, "zhong");
    let focus = device.0.borrow().focus.last().unwrap().1.clone().unwrap();
    assert_eq!(focus.text, "A中");
    assert_eq!(
        focus.selection,
        NativeSelection {
            anchor: 1,
            focus: 2
        }
    );
    assert!(focus.caret.height > 0, "{focus:?}");
    send(
        &device,
        Native::Commit {
            id,
            text: "钟".into(),
        },
    );
    session.tick().unwrap();
    assert_eq!(evaluate(&mut session, "title.getValue();"), "A钟");

    send(
        &device,
        Native::Mark {
            id: NativeId(25),
            text: "temporary".into(),
            selection: NativeSelection::caret(9),
        },
    );
    session.tick().unwrap();
    send(&device, Native::Cancel { id: NativeId(25) });
    session.tick().unwrap();
    assert_eq!(evaluate(&mut session, "title.getValue();"), "A钟");
    evaluate(&mut session, "document.getElementById('add').focus();");
    session.tick().unwrap();
    assert!(device.0.borrow().focus.last().unwrap().1.is_none());
    call(&mut session, Command::Shutdown {});
}
