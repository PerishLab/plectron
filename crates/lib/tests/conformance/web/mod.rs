use super::device::{self, Device};
use plectron::addons::control::Pump;
use plectron::contract::Unit;
use plectron::ipc::{Command, Reply, Request, Session};
use plectron::platform::Host;
use plectron::rendering::Solid;
use webengine::{Snapshot, State};

fn setup() -> (Session<Device, Solid>, Device) {
    let (mut runtime, device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/web/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    (Session::new(runtime, 42, false), device)
}

pub(super) fn call(session: &mut Session<Device, Solid>, command: Command) -> Reply {
    session.handle(Request::new(command)).result.unwrap()
}

pub(super) fn snapshot(session: &mut Session<Device, Solid>) -> Snapshot {
    let Reply::Page { snapshot } = call(session, Command::Page { unit: 1 }) else {
        panic!()
    };
    *snapshot
}

pub(super) fn point(snapshot: &Snapshot, id: &str) -> (usize, usize) {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|value| value == id))
        .unwrap();
    let rect = snapshot
        .fragments
        .iter()
        .find(|item| item.node == Some(node.id))
        .unwrap()
        .rect;
    (rect.x + 4, rect.y + 4)
}

#[test]
fn application() {
    let (mut session, device) = setup();
    let first = snapshot(&mut session);
    assert_eq!(first.state, State::Ready);
    assert_eq!(first.url, "app://demo/site/index.html");
    assert_eq!(first.resources.len(), 4);
    assert!(
        device
            .0
            .borrow()
            .titles
            .last()
            .unwrap()
            .contains("app://demo/site/index.html")
    );
    let frames = device.0.borrow().frames.len();
    let before = point(&first, "reset");
    let (x, y) = point(&first, "increment");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    session.tick().unwrap();
    let changed = snapshot(&mut session);
    assert!(changed.nodes.iter().any(|node| node.content == "Count: 1"));
    assert!(point(&changed, "reset").1 > before.1);
    assert_eq!(device.0.borrow().frames.len(), frames + 1);
    session.tick().unwrap();
    assert_eq!(
        device.0.borrow().titles.last().unwrap(),
        "Plectron - count: 1"
    );
    call(
        &mut session,
        Command::Send {
            unit: 1,
            text: "reset".into(),
        },
    );
    session.tick().unwrap();
    session.tick().unwrap();
    assert!(
        snapshot(&mut session)
            .nodes
            .iter()
            .any(|node| node.content == "Count: 0")
    );
    assert_eq!(
        device.0.borrow().titles.last().unwrap(),
        "Plectron - count: 0"
    );
    let Reply::Services { entries } = call(&mut session, Command::Services {}) else {
        panic!()
    };
    let address = entries[0].address.clone();
    let Reply::Shutdown { errors } = call(&mut session, Command::Shutdown {}) else {
        panic!()
    };
    assert!(errors.is_empty(), "{errors:?}");
    assert!(device.0.borrow().windows.is_empty());
    assert!(!session.active());
    assert!(std::net::TcpStream::connect(address.strip_prefix("http://").unwrap()).is_err());
}

#[test]
fn debugging() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    let (x, y) = point(&snapshot(&mut session), "increment");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    session.tick().unwrap();
    assert!(
        snapshot(&mut session)
            .nodes
            .iter()
            .any(|node| node.content == "Count: 0")
    );
    call(&mut session, Command::Step {});
    assert!(
        snapshot(&mut session)
            .nodes
            .iter()
            .any(|node| node.content == "Count: 1")
    );
    assert!(device.0.borrow().titles.last().unwrap().contains("app://"));
    session.tick().unwrap();
    call(&mut session, Command::Step {});
    assert_eq!(
        device.0.borrow().titles.last().unwrap(),
        "Plectron - count: 1"
    );
    let response = call(
        &mut session,
        Command::Evaluate {
            unit: 1,
            source: "count;".into(),
        },
    );
    assert!(matches!(response, Reply::Evaluation { display } if display == "1"));
    assert!(
        session
            .handle(Request::new(Command::Eval {
                source: "count;".into()
            }))
            .result
            .is_err()
    );
    assert!(
        session
            .handle(Request::new(Command::Eval {
                source: "window.fill(0);".into()
            }))
            .result
            .is_err()
    );
    assert!(
        session
            .handle(Request::new(Command::Eval {
                source: "window.load('app://demo/site/index.html');".into()
            }))
            .result
            .is_err()
    );
    call(&mut session, Command::Close { unit: 1 });
    assert!(
        session
            .handle(Request::new(Command::Evaluate {
                unit: 1,
                source: "count;".into()
            }))
            .result
            .is_err()
    );
    assert!(
        session
            .handle(Request::new(Command::Send {
                unit: 1,
                text: "late".into()
            }))
            .result
            .is_err()
    );
    call(&mut session, Command::Shutdown {});
}

#[test]
fn failure() {
    let (mut runtime, device) = device::control();
    runtime.execute("const win = platform.window({}); win.on('error', function (error) { console.log(error); }); win.load('app://missing/index.html');").unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    let snapshot = snapshot(&mut session);
    assert_eq!(snapshot.state, State::Failed);
    assert!(snapshot.fault.unwrap().contains("unregistered"));
    assert!(device.0.borrow().frames.is_empty());
    assert!(device.0.borrow().logs[0].contains("unregistered"));
    assert!(session.active());
    call(&mut session, Command::Shutdown {});
    assert!(device.0.borrow().windows.is_empty());
}

#[test]
fn activation() {
    let (mut runtime, device) = device::control();
    runtime.execute("const win = platform.window({});").unwrap();
    device.0.borrow_mut().reject = Some("activate");
    let failure = runtime.execute("win.load('app://missing/index.html');");
    assert!(failure.is_err());
    assert!(!runtime.addons().host.attached(Unit::new(1)));
    assert!(device.0.borrow().activated.is_empty());
}

#[test]
fn caches() {
    let (mut session, _) = setup();
    let before = snapshot(&mut session);
    for name in ["paint", "fragments", "subsequences"] {
        assert!(before.caches.iter().any(|cache| cache.name == name));
    }
    let id = before
        .caches
        .iter()
        .find(|cache| cache.name == "paint")
        .unwrap()
        .id;
    let command = Command::Cache {
        unit: 1,
        id,
        command: shared::cache::Command::Destroy,
    };
    let reply = call(&mut session, command);
    assert!(matches!(reply, Reply::Evaluation { .. }));
    let after = snapshot(&mut session);
    assert!(
        after
            .caches
            .iter()
            .find(|cache| cache.id == id)
            .unwrap()
            .closed
    );
    assert_eq!(after.scene, before.scene);
    assert_eq!(after.fragments, before.fragments);
    assert_eq!(after.update.committed, before.update.committed);
    assert_eq!(after.painting.revision, before.painting.revision);
    assert!(
        session
            .handle(Request::new(Command::Cache {
                unit: 1,
                id,
                command: shared::cache::Command::Clear
            }))
            .result
            .is_err()
    );
}

mod transitions;
