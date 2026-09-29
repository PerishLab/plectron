use super::evaluate;
use crate::{
    device,
    web::{call, point, snapshot},
};
use plectron::{
    addons::control::Pump,
    ipc::{Command, Session},
    platform::Host,
};
use webengine::Snapshot;

fn identity(snapshot: &Snapshot, name: &str) -> usize {
    snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == name))
        .unwrap()
        .id
}
fn space(snapshot: &Snapshot, name: &str) -> u64 {
    let id = identity(snapshot, name);
    let boxspace = snapshot
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(id))
        .unwrap()
        .space;
    snapshot
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(boxspace) && mapping.range.is_some())
        .unwrap()
        .id
}
#[test]
fn interaction() {
    let (mut runtime, device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/flex/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    let first = snapshot(&mut session);
    assert_eq!(first.state, webengine::State::Ready);
    assert_eq!(first.url, "app://flex/flex/index.html");
    assert_eq!(first.resources.len(), 4);
    let retained = identity(&first, "row-1-0");
    let (x, y) = point(&first, "row-1-0");
    call(&mut session, Command::Pause {});
    call(&mut session, Command::Pointer { unit: 1, x, y });
    evaluate(&mut session, "add();");
    let added = snapshot(&mut session);
    assert_eq!(identity(&added, "row-1-0"), retained);
    assert!(point(&added, "row-1-0").0 < x);
    assert_eq!(evaluate(&mut session, "panels.length;"), "3");
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "selected;"), "1:0");
    call(&mut session, Command::Resume {});
    session.tick().unwrap();
    session.tick().unwrap();
    assert_eq!(
        device.0.borrow().titles.last().unwrap(),
        "Plectron - selected:1:0"
    );
    let (x, y) = point(&snapshot(&mut session), "remove");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    session.tick().unwrap();
    let reduced = snapshot(&mut session);
    assert_eq!(evaluate(&mut session, "panels.length;"), "2");
    assert!(
        !reduced
            .nodes
            .iter()
            .any(|node| node.attributes.get("id").is_some_and(|id| id == "panel-2"))
    );
    assert_eq!(point(&reduced, "row-1-0").0, point(&first, "row-1-0").0);
    let rows = space(&reduced, "rows-0");
    call(&mut session, Command::Pause {});
    let (x, y) = point(&reduced, "row-0-0");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    call(
        &mut session,
        Command::Pan {
            unit: 1,
            scene: reduced.scene.unwrap(),
            space: rows,
            delta: 72,
        },
    );
    let scrolled = snapshot(&mut session);
    assert_eq!(scrolled.scene, reduced.scene);
    assert_eq!(scrolled.painted, reduced.painted);
    assert_eq!(scrolled.shown.unwrap().offset(rows), 72);
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "selected;"), "0:0");
    call(&mut session, Command::Shutdown {});
    assert!(device.0.borrow().windows.is_empty());
}
