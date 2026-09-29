use super::{complete, evaluate};
use crate::{
    device::{self, Device},
    web::{call, snapshot},
};
use plectron::{
    addons::control::Pump,
    ipc::{Command, Reply, Session},
    platform::Host,
    platform::device::{Event, Signal},
    rendering::Solid,
};

fn setup() -> (Session<Device, Solid>, Device) {
    let (mut runtime, device) = device::runtime();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
    let source = crate::application("apps/scroll/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    (Session::new(runtime, 42, false), device)
}
fn point(snapshot: &webengine::Snapshot, name: &str) -> (usize, usize) {
    let node = snapshot
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == name))
        .unwrap();
    let fragment = snapshot
        .visible
        .iter()
        .find(|fragment| fragment.node == Some(node.id) && fragment.rect.height > 8)
        .unwrap();
    (fragment.rect.x + 8, fragment.rect.y + 8)
}

#[test]
fn paused() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    let before = snapshot(&mut session);
    let (x, y) = point(&before, "first");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    device.0.borrow_mut().deferred = true;
    call(
        &mut session,
        Command::Scroll {
            unit: 1,
            delta: 100,
        },
    );
    for _ in 0..10 {
        session.tick().unwrap();
    }
    let pending = snapshot(&mut session);
    assert_eq!(pending.scene, before.scene);
    assert_eq!(pending.frame, before.frame);
    assert_eq!(pending.visual.offset(0), 100);
    assert_eq!(evaluate(&mut session, "clicks;"), "0");
    complete(&device, 0);
    session.tick().unwrap();
    let shown = snapshot(&mut session);
    assert_eq!(shown.shown.as_ref().unwrap().offset(0), 100);
    assert_ne!(shown.frame, before.frame);
    assert_eq!(shown.painted, before.painted);
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "last;"), "first");
    let (x, y) = point(&shown, "second");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "last;"), "second");
    call(&mut session, Command::Resume {});
    session.tick().unwrap();
    assert_eq!(snapshot(&mut session).shown.unwrap().offset(0), 100);
    call(&mut session, Command::Shutdown {});
}

#[test]
fn wheel() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    let before = snapshot(&mut session);
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Scroll { delta: 80 },
    });
    session.tick().unwrap();
    let after = snapshot(&mut session);
    assert_eq!(after.shown.unwrap().offset(0), 80);
    assert_eq!(before.scene, after.scene);
    assert_eq!(evaluate(&mut session, "clicks;"), "0");
    assert!(matches!(
        call(&mut session, Command::Status {}),
        Reply::Status { queued: 0, .. }
    ));
    call(&mut session, Command::Shutdown {});
}

#[test]
fn controlledwheel() {
    let (mut session, _) = setup();
    call(&mut session, Command::Pause {});
    let before = snapshot(&mut session);
    call(
        &mut session,
        Command::Wheel {
            unit: 1,
            x: 180,
            y: 120,
            delta: 80,
        },
    );
    let after = snapshot(&mut session);
    assert_eq!(before.scene, after.scene);
    assert_eq!(after.visual.offset(0), 80);
    call(&mut session, Command::Shutdown {});
}
