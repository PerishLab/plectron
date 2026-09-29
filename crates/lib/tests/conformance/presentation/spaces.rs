use super::evaluate;
use crate::{
    device,
    web::{call, snapshot},
};
use plectron::{
    addons::control::Pump,
    ipc::{Command, Request, Session},
    platform::{
        Host,
        device::{Event, Point, Signal},
    },
};

#[test]
fn paused() {
    let (mut runtime, device) = device::runtime();
    let root = fixtures::root();
    let source = crate::application("apps/spaces/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    runtime.poll().unwrap();
    let mut session = Session::new(runtime, 42, false);
    call(&mut session, Command::Pause {});
    let before = snapshot(&mut session);
    let node = before
        .nodes
        .iter()
        .find(|node| node.attributes.get("id").is_some_and(|id| id == "inner"))
        .unwrap();
    let boxspace = before
        .fragments
        .iter()
        .find(|fragment| fragment.node == Some(node.id))
        .unwrap()
        .space;
    let inner = before
        .spaces
        .iter()
        .find(|mapping| mapping.parent == Some(boxspace) && mapping.range.is_some())
        .unwrap()
        .id;
    call(
        &mut session,
        Command::Pointer {
            unit: 1,
            x: 40,
            y: 130,
        },
    );
    call(
        &mut session,
        Command::Pan {
            unit: 1,
            scene: before.scene.unwrap(),
            space: inner,
            delta: 40,
        },
    );
    let after = snapshot(&mut session);
    assert_eq!(after.scene, before.scene);
    assert_eq!(after.painted, before.painted);
    assert_eq!(after.shown.unwrap().offset(inner), 40);
    assert_eq!(evaluate(&mut session, "clicks;"), "0");
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "last;"), "a");
    let current = snapshot(&mut session);
    assert!(
        session
            .handle(Request::new(Command::Pan {
                unit: 1,
                scene: before.scene.unwrap(),
                space: inner,
                delta: 1
            }))
            .result
            .is_err()
    );
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Wheel {
            position: Point { x: 40, y: 130 },
            delta: 16,
        },
    });
    session.tick().unwrap();
    session.tick().unwrap();
    let wheel = snapshot(&mut session);
    assert_eq!(wheel.scene, current.scene);
    assert_eq!(wheel.shown.unwrap().offset(inner), 56);
    assert_eq!(evaluate(&mut session, "clicks;"), "1");
    call(&mut session, Command::Shutdown {});
}
