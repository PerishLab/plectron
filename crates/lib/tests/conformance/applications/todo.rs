use crate::device::{self, Device};
use crate::web::{call, point, snapshot};
use plectron::addons::control::Pump;
use plectron::ipc::{Command, Reply, Session};
use plectron::platform::{
    Host,
    device::{Event, Phase, Signal},
};
use plectron::rendering::Solid;
use webengine::{Snapshot, State};
mod sustained;

fn setup() -> (Session<Device, Solid>, Device) {
    let (mut runtime, device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/todo/start.ts")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let page = snapshot(&mut session);
        match page.state {
            State::Ready => return (session, device),
            State::Failed => panic!("{}", page.fault.unwrap()),
            _ => (),
        }
        assert!(std::time::Instant::now() < deadline);
        session.tick().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
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

fn tap(session: &mut Session<Device, Solid>, device: &Device, id: &str) -> Snapshot {
    let (x, y) = point(&snapshot(session), id);
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device
        .0
        .borrow_mut()
        .signals
        .extend(device::tap(unit, Some(device::at(x, y))));
    loop {
        session.tick().unwrap();
        if device.0.borrow().signals.is_empty() {
            break;
        }
    }
    snapshot(session)
}

fn key(chord: &str) -> Command {
    use webengine::keyboard::{Chord, Input, Stroke};
    Command::Input {
        unit: 1,
        input: Input::Key(Stroke {
            chord: Chord::parse(chord).unwrap(),
            text: String::new(),
            repeat: false,
        }),
    }
}

fn class(snapshot: &Snapshot, id: &str) -> String {
    snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .unwrap()
        .attributes["class"]
        .clone()
}

fn rows(snapshot: &Snapshot) -> usize {
    snapshot
        .nodes
        .iter()
        .filter(|node| {
            node.attributes
                .get("id")
                .is_some_and(|id| id.starts_with("todo-"))
        })
        .count()
}

fn text(snapshot: &Snapshot, id: &str) -> String {
    let parent = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .unwrap()
        .id;
    snapshot
        .nodes
        .iter()
        .filter(|node| node.parent == Some(parent))
        .map(|node| node.content.as_str())
        .collect()
}

#[test]
fn typing() {
    let (mut session, device) = setup();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    assert_eq!(
        device
            .0
            .borrow()
            .focus
            .last()
            .unwrap()
            .1
            .as_ref()
            .unwrap()
            .text,
        ""
    );
    for _ in 0..2 {
        device.0.borrow_mut().signals.push(Signal {
            unit,
            event: Event::Input(webengine::keyboard::Input::Text("f".into())),
        });
        session.tick().unwrap();
    }
    assert_eq!(
        evaluate(
            &mut session,
            "document.getElementById('task-input').getValue();"
        ),
        "ff"
    );
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Input(webengine::keyboard::Input::Text("中文".into())),
    });
    session.tick().unwrap();
    assert_eq!(
        evaluate(
            &mut session,
            "document.getElementById('task-input').getValue();"
        ),
        "ff中文"
    );
    assert_eq!(
        device
            .0
            .borrow()
            .focus
            .last()
            .unwrap()
            .1
            .as_ref()
            .unwrap()
            .text,
        "ff中文"
    );
    call(&mut session, key("Backspace"));
    session.tick().unwrap();
    assert_eq!(
        evaluate(
            &mut session,
            "document.getElementById('task-input').getValue();"
        ),
        "ff中"
    );
    assert_eq!(rows(&snapshot(&mut session)), 3);
}

#[test]
fn application() {
    let (mut session, device) = setup();
    let first = snapshot(&mut session);
    assert!(
        first
            .resources
            .iter()
            .any(|resource| resource.url.ends_with("/todo/main.ts")
                && resource.mime == "text/javascript")
    );
    assert_eq!(rows(&first), 3);
    assert_eq!(text(&first, "summary"), "2 remaining / 3 total");
    assert_eq!(
        evaluate(
            &mut session,
            "document.focused() === document.getElementById('task-input');"
        ),
        "true"
    );
    assert!(class(&first, "task-input").contains("bg-selected"));

    let (x, y) = point(&first, "todo-1");
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Pointer {
            position: Some(device::at(x, y)),
            phase: Phase::Move,
        },
    });
    session.tick().unwrap();
    assert!(class(&snapshot(&mut session), "todo-1").contains("bg-hover"));
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Pointer {
            position: None,
            phase: Phase::Move,
        },
    });
    session.tick().unwrap();
    assert!(class(&snapshot(&mut session), "todo-1").contains("bg-input"));

    call(&mut session, key("Tab"));
    session.tick().unwrap();
    assert_eq!(
        evaluate(
            &mut session,
            "document.focused() === document.getElementById('add');"
        ),
        "true"
    );
    let focused = snapshot(&mut session);
    assert!(class(&focused, "task-input").contains("bg-input"));
    assert!(class(&focused, "add").contains("bg-selected"));
    evaluate(
        &mut session,
        "const input = document.getElementById('task-input'); input.setValue('Exercise the real application'); input.focus();",
    );
    let focused = snapshot(&mut session);
    assert!(class(&focused, "task-input").contains("bg-selected"));
    assert!(class(&focused, "add").contains("bg-primary"));
    call(&mut session, key("Enter"));
    session.tick().unwrap();
    let added = snapshot(&mut session);
    assert_eq!(rows(&added), 4);
    assert_eq!(text(&added, "summary"), "3 remaining / 4 total");
    assert!(
        added
            .nodes
            .iter()
            .any(|node| node.content == "Exercise the real application")
    );

    tap(&mut session, &device, "toggle-1");
    assert!(class(&snapshot(&mut session), "todo-1").contains("task-done"));
    assert_eq!(
        text(&snapshot(&mut session), "summary"),
        "2 remaining / 4 total"
    );
    let identity = snapshot(&mut session)
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == "todo-1"))
        .unwrap()
        .id;
    tap(&mut session, &device, "filter-active");
    assert!(class(&snapshot(&mut session), "todo-1").contains("hidden"));
    assert_eq!(
        snapshot(&mut session)
            .nodes
            .iter()
            .find(|node| node.attributes.get("id").is_some_and(|id| id == "todo-1"))
            .unwrap()
            .id,
        identity
    );
    tap(&mut session, &device, "filter-all");
    tap(&mut session, &device, "remove-1");
    assert_eq!(rows(&snapshot(&mut session)), 3);
    assert_eq!(
        text(&snapshot(&mut session), "summary"),
        "2 remaining / 3 total"
    );
    assert!(
        snapshot(&mut session)
            .nodes
            .iter()
            .all(|node| node.attributes.get("id").is_none_or(|id| id != "todo-1"))
    );
    tap(&mut session, &device, "clear");
    assert_eq!(rows(&snapshot(&mut session)), 2);
    assert_eq!(
        text(&snapshot(&mut session), "summary"),
        "2 remaining / 2 total"
    );
}
