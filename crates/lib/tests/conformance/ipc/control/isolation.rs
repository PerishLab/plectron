use super::{Debugger, job, start};
use crate::web::{call, point, snapshot};
use plectron::{
    control::Phase,
    ipc::Command,
    platform::device::{Event, Point, Signal},
};

#[test]
fn independent() {
    let _guard = super::serial();
    let root = fixtures::root();
    let source = crate::application("apps/spaces/start.ts")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    let (mut session, device) = start(&source);
    session.until(|session| {
        snapshot(session).frame.is_some()
            && device
                .0
                .borrow()
                .logs
                .iter()
                .any(|text| text == "spaces ready")
    });
    let held = job(call(&mut session, Command::Hold { unit: None }));
    session.until(|session| session.phase(held) == Phase::Held);
    let content = job(call(&mut session, Command::Hold { unit: Some(1) }));
    session.until(|session| {
        snapshot(session)
            .execution
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == content && job.phase() == webengine::Phase::Held)
    });
    let queued = job(call(
        &mut session,
        Command::Eval {
            source: "42;".into(),
        },
    ));
    let before = snapshot(&mut session);
    call(&mut session, Command::Scroll { unit: 1, delta: 24 });
    let scrolled = snapshot(&mut session);
    assert_eq!(scrolled.scene, before.scene);
    assert_ne!(scrolled.frame, before.frame);
    assert_eq!(scrolled.shown.unwrap().offset(0), 24);
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Wheel {
            position: Point { x: 40, y: 110 },
            delta: 16,
        },
    });
    session.until(|session| snapshot(session).visual.revision > scrolled.visual.revision);
    assert_eq!(session.phase(held), Phase::Held);
    assert_eq!(session.phase(queued), Phase::Queued);
    call(&mut session, Command::Unhold { unit: Some(1) });
    call(
        &mut session,
        Command::Scroll {
            unit: 1,
            delta: -24,
        },
    );
    let (x, y) = point(&snapshot(&mut session), "before");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    session.until(|session| {
        snapshot(session)
            .nodes
            .iter()
            .any(|node| node.content == "before / 1")
    });
    assert!(device.0.borrow().titles.is_empty());
    assert_eq!(session.phase(held), Phase::Held);
    call(&mut session, Command::Unhold { unit: None });
    assert_eq!(session.complete(queued).unwrap(), "42");
    session.until(|_| {
        device
            .0
            .borrow()
            .titles
            .last()
            .is_some_and(|text| text == "before / 1")
    });
    assert!(
        session
            .inspect()
            .jobs
            .iter()
            .all(|job| job.result().is_none_or(|result| result.is_ok()))
    );
    call(&mut session, Command::Shutdown {});
}
