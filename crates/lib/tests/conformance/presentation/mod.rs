mod flex;
mod scrolling;
mod spaces;
mod worker;
use super::{
    device::{self, Device},
    web::{call, point, snapshot},
};
use plectron::{
    ipc::{Command, Reply, Request, Session},
    platform::{Host, Receipt},
    rendering::Solid,
};

fn setup() -> (Session<Device, Solid>, Device) {
    let (mut runtime, device) = device::runtime();
    device.0.borrow_mut().deferred = true;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
    let source = crate::application("apps/web/start.js")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    (Session::new(runtime, 42, false), device)
}
fn complete(device: &Device, index: usize) {
    let mut record = device.0.borrow_mut();
    let (unit, scene) = &record.submissions[index];
    let receipt = Receipt {
        produced: None,
        visible: None,
        drops: 0,
        unit: *unit,
        frame: scene.id(),
        result: Ok(()),
    };
    record.receipts.push(receipt);
}
fn evaluate(session: &mut Session<Device, Solid>, source: &str) -> String {
    let Reply::Evaluation { display } = call(
        session,
        Command::Evaluate {
            unit: 1,
            source: source.into(),
        },
    ) else {
        panic!();
    };
    display
}

#[test]
fn delayed() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    let first = snapshot(&mut session);
    assert!(first.presented.is_none());
    let (x, y) = point(&first, "increment");
    call(&mut session, Command::Pointer { unit: 1, x, y });
    for _ in 0..20 {
        session.tick().unwrap();
    }
    assert_eq!(device.0.borrow().submissions.len(), 1);
    assert!(matches!(
        call(&mut session, Command::Status {}),
        Reply::Status { queued: 1, .. }
    ));
    complete(&device, 0);
    session.tick().unwrap();
    assert_eq!(snapshot(&mut session).frame, first.issued);
    assert!(device.0.borrow().titles.is_empty());
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "count;"), "0");
    call(&mut session, Command::Step {});
    assert!(!device.0.borrow().titles.is_empty());
    evaluate(
        &mut session,
        "increment.setAttribute('style', 'height: 80px;');",
    );
    let second = snapshot(&mut session);
    assert_eq!(second.presented, first.painted);
    for height in [90, 100, 110] {
        evaluate(
            &mut session,
            &format!("increment.setAttribute('style', 'height: {height}px;');"),
        );
    }
    let latest = snapshot(&mut session);
    assert_eq!(device.0.borrow().submissions.len(), 2);
    assert_ne!(latest.pending, second.issued);
    complete(&device, 1);
    session.tick().unwrap();
    assert_eq!(snapshot(&mut session).presented, second.painted);
    assert_eq!(snapshot(&mut session).issued, latest.pending);
    assert_eq!(device.0.borrow().submissions.len(), 3);
    complete(&device, 2);
    session.tick().unwrap();
    assert_eq!(snapshot(&mut session).presented, latest.painted);
    call(&mut session, Command::Shutdown {});
}

#[test]
fn targeting() {
    let (mut session, device) = setup();
    complete(&device, 0);
    session.tick().unwrap();
    call(&mut session, Command::Pause {});
    let (x, y) = point(&snapshot(&mut session), "reset");
    evaluate(
        &mut session,
        "count = 10; increment.setAttribute('style', 'height: 240px;');",
    );
    call(&mut session, Command::Pointer { unit: 1, x, y });
    complete(&device, 1);
    session.tick().unwrap();
    call(&mut session, Command::Step {});
    assert_eq!(evaluate(&mut session, "count;"), "0");
    call(&mut session, Command::Shutdown {});
}

#[test]
fn failure() {
    let (mut session, device) = setup();
    complete(&device, 0);
    session.tick().unwrap();
    let first = snapshot(&mut session);
    evaluate(
        &mut session,
        "increment.setAttribute('style', 'height: 80px;');",
    );
    complete(&device, 1);
    device.0.borrow_mut().receipts[0].result = Err(plectron::contract::Fault("GPU stopped".into()));
    call(&mut session, Command::Pause {});
    session.tick().unwrap();
    let failed = snapshot(&mut session);
    assert_eq!(failed.frame, first.frame);
    assert_eq!(failed.output.as_deref(), Some("GPU stopped"));
    assert!(failed.issued.is_none());
    assert_eq!(evaluate(&mut session, "count = 9; count;"), "9");
    for _ in 0..10 {
        session.tick().unwrap();
    }
    assert_eq!(device.0.borrow().submissions.len(), 2);
    call(&mut session, Command::Close { unit: 1 });
    call(&mut session, Command::Shutdown {});
    assert!(device.0.borrow().windows.is_empty());
}

#[test]
fn closing() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    call(&mut session, Command::Close { unit: 1 });
    complete(&device, 0);
    session.tick().unwrap();
    assert!(
        session
            .handle(Request::new(Command::Page { unit: 1 }))
            .result
            .is_err()
    );
    assert!(device.0.borrow().windows.is_empty());
    assert!(device.0.borrow().titles.is_empty());
    call(&mut session, Command::Shutdown {});
}

#[test]
fn interrupted() {
    let (mut session, device) = setup();
    complete(&device, 0);
    session.tick().unwrap();
    let first = snapshot(&mut session);
    assert!(
        session
            .handle(Request::new(Command::Evaluate {
                unit: 1,
                source: "increment.setAttribute('style', 'height: 80px;'); absent();".into(),
            }))
            .result
            .is_err()
    );
    for _ in 0..10 {
        session.tick().unwrap();
    }
    let failed = snapshot(&mut session);
    assert!(failed.revision > failed.painted);
    assert_eq!(failed.frame, first.frame);
    assert_eq!(device.0.borrow().submissions.len(), 1);
    call(&mut session, Command::Shutdown {});
}
