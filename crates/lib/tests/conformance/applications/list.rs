use crate::device::{self, Device};
use crate::web::{call, point, snapshot};
use plectron::addons::control::Pump;
use plectron::ipc::{Command, Reply, Session};
use plectron::platform::Host;
use plectron::rendering::Solid;
use webengine::{Snapshot, State};

fn setup() -> (Session<Device, Solid>, Device) {
    let (mut runtime, device) = device::runtime();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
    let source = crate::application("apps/list/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    (Session::new(runtime, 42, false), device)
}

fn tap(session: &mut Session<Device, Solid>, id: &str) -> Snapshot {
    let (x, y) = point(&snapshot(session), id);
    call(session, Command::Pointer { unit: 1, x, y });
    session.tick().unwrap();
    session.tick().unwrap();
    snapshot(session)
}

fn identity(snapshot: &Snapshot, id: &str) -> Option<usize> {
    snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .map(|node| node.id)
}

fn evaluate(session: &mut Session<Device, Solid>, source: &str) -> String {
    let Reply::Evaluation { display } = call(
        session,
        Command::Evaluate {
            unit: 1,
            source: source.into(),
        },
    ) else {
        panic!()
    };
    display
}

#[test]
fn interaction() {
    let (mut session, device) = setup();
    let first = snapshot(&mut session);
    assert_eq!(first.state, State::Ready, "{:?}", first.fault);
    assert_eq!(first.url, "app://tasks/list/index.html");
    assert_eq!(first.resources.len(), 5);
    assert_eq!(evaluate(&mut session, "items.length;"), "3");
    let retained = identity(&first, "task-1").unwrap();
    let pixels = device.0.borrow().frames.last().unwrap().clone();
    let toggled = tap(&mut session, "task-0");
    assert_eq!(toggled.revision, toggled.painted);
    assert_eq!(identity(&toggled, "task-0"), identity(&first, "task-0"));
    assert!(
        toggled
            .nodes
            .iter()
            .any(|node| node.content == "Done: Model boundaries")
    );
    assert_eq!(evaluate(&mut session, "items[1].done;"), "false");
    assert_ne!(device.0.borrow().frames.last().unwrap(), &pixels);
    tap(&mut session, "task-2");
    let removed = tap(&mut session, "remove");
    assert_eq!(identity(&removed, "task-0"), None);
    assert_eq!(identity(&removed, "task-2"), None);
    assert_eq!(identity(&removed, "task-1"), Some(retained));
    assert!(point(&removed, "task-1").1 < point(&first, "task-1").1);
    assert_eq!(evaluate(&mut session, "items.length;"), "1");
    assert_eq!(
        device.0.borrow().titles.last().unwrap(),
        "Plectron - remaining:1"
    );
    let added = tap(&mut session, "add");
    assert!(identity(&added, "task-3").is_some());
    assert_eq!(evaluate(&mut session, "items[1].id;"), "3");
    tap(&mut session, "task-3");
    assert_eq!(evaluate(&mut session, "items[1].done;"), "true");
    call(
        &mut session,
        Command::Send {
            unit: 1,
            text: "reset".into(),
        },
    );
    session.tick().unwrap();
    assert_eq!(evaluate(&mut session, "items[1].done;"), "false");
    assert!(
        snapshot(&mut session)
            .nodes
            .iter()
            .any(|node| node.content == "Reset")
    );
    let Reply::Shutdown { errors } = call(&mut session, Command::Shutdown {}) else {
        panic!()
    };
    assert!(errors.is_empty(), "{errors:?}");
    assert!(!session.active());
    assert!(device.0.borrow().windows.is_empty());
}

#[test]
fn editing() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    for edit in [
        webengine::Edit::Insert("Draft title".into()),
        webengine::Edit::All,
        webengine::Edit::Insert("Modern Z".into()),
        webengine::Edit::Backward,
        webengine::Edit::Insert("browser".into()),
        webengine::Edit::Commit,
    ] {
        call(&mut session, Command::Edit { unit: 1, edit });
    }
    assert_eq!(evaluate(&mut session, "items.length;"), "3");
    for _ in 0..6 {
        call(&mut session, Command::Step {});
    }
    assert_eq!(evaluate(&mut session, "items[3].title;"), "Modern browser");
    assert_eq!(evaluate(&mut session, "title.getValue();"), "");
    assert_eq!(
        evaluate(&mut session, "document.focused() === title;"),
        "true"
    );
    let first = snapshot(&mut session);
    let list = identity(&first, "list").unwrap();
    let space = first
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(list))
        .unwrap()
        .space
        + 1;
    let (x, y) = point(&first, "task-0");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    call(
        &mut session,
        Command::Pan {
            unit: 1,
            scene: first.scene.unwrap(),
            space,
            delta: 100,
        },
    );
    let scrolled = snapshot(&mut session);
    assert_eq!(scrolled.scene, first.scene);
    assert!(scrolled.visual.offset(space) > 0);
    assert_eq!(evaluate(&mut session, "items[0].done;"), "false");
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "items[0].done;"), "true");
    assert_eq!(
        evaluate(
            &mut session,
            "document.focused() === document.getElementById('task-0');"
        ),
        "true"
    );
    call(
        &mut session,
        Command::Edit {
            unit: 1,
            edit: webengine::Edit::Insert("ignored".into()),
        },
    );
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "title.getValue();"), "");
    call(&mut session, Command::Resume {});
    session.tick().unwrap();
    assert!(!device.0.borrow().frames.is_empty());
    call(&mut session, Command::Shutdown {});
}

mod composition;
fn key(chord: &str, text: &str) -> Command {
    use webengine::keyboard::{Chord, Input, Stroke};
    Command::Input {
        unit: 1,
        input: Input::Key(Stroke {
            chord: Chord::parse(chord).unwrap(),
            text: text.into(),
            repeat: false,
        }),
    }
}

#[test]
fn shortcuts() {
    let (mut session, _) = setup();
    call(&mut session, Command::Pause {});
    evaluate(&mut session, "title.setValue('draft');");
    call(&mut session, key("Primary+KeyK", "k"));
    call(
        &mut session,
        Command::Input {
            unit: 1,
            input: webengine::keyboard::Input::Text("Bound command".into()),
        },
    );
    call(&mut session, key("Primary+Enter", ""));
    assert_eq!(evaluate(&mut session, "items.length;"), "3");
    for _ in 0..3 {
        call(&mut session, Command::Step {});
    }
    assert_eq!(evaluate(&mut session, "items[3].title;"), "Bound command");
    call(&mut session, key("Primary+Enter", ""));
    evaluate(&mut session, "bindings.dispose();");
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "items.length;"), "4");
    call(&mut session, key("Enter", ""));
    call(&mut session, Command::Close { unit: 1 });
    call(&mut session, Command::Step {});
    let Reply::Status { windows, fault, .. } = call(&mut session, Command::Status {}) else {
        panic!()
    };
    assert_eq!(windows, 0);
    assert!(fault.is_none());
    assert!(
        session
            .handle(plectron::ipc::Request::new(key("Enter", "")))
            .result
            .is_err()
    );
    call(&mut session, Command::Shutdown {});
    assert!(!session.active());
}

#[test]
fn subscriptions() {
    let (mut session, _) = setup();
    evaluate(&mut session, "const removed = items[0];");
    tap(&mut session, "task-0");
    tap(&mut session, "remove");
    assert_eq!(evaluate(&mut session, "removed.done;"), "true");
    evaluate(&mut session, "removed.node.dispatchEvent('click', '');");
    assert_eq!(evaluate(&mut session, "removed.done;"), "true");
    let snapshot = snapshot(&mut session);
    let record = snapshot.events.last().unwrap();
    assert!(record.deliveries.is_empty());
    assert_eq!(record.origin, webengine::Origin::Script);
    evaluate(
        &mut session,
        "title.focus(); title.addEventListener('beforeinput', () => { throw Error('diagnostic'); });",
    );
    call(&mut session, Command::Pause {});
    call(
        &mut session,
        Command::Input {
            unit: 1,
            input: webengine::keyboard::Input::Text("visible".into()),
        },
    );
    call(&mut session, Command::Step {});
    let snapshot = self::snapshot(&mut session);
    let record = snapshot
        .events
        .iter()
        .rev()
        .find(|record| record.kind == "beforeinput")
        .unwrap();
    assert_eq!(record.action, webengine::Effect::Applied);
    assert_eq!(record.errors, 1);
    assert!(record.exception.as_ref().unwrap().contains("diagnostic"));
    assert_eq!(evaluate(&mut session, "title.getValue();"), "visible");
    call(&mut session, Command::Shutdown {});
}
