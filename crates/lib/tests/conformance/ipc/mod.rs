use plectron::ipc::{Command, Mode, Reply, Request, Session};
use plectron::rendering::Solid;

use super::device::{self, Device};

fn setup() -> (Session<Device, Solid>, Device) {
    let (mut runtime, device) = device::control();
    runtime.execute("let count = 0; let win = platform.window({title: 'before'}); win.on('click', function () { count = count + 1; win.title('after'); }); win.on('close', function () { count = count + 100; });").unwrap();
    (Session::new(runtime, 42, false), device)
}

fn call(session: &mut Session<Device, Solid>, command: Command) -> Reply {
    session.handle(Request::new(command)).result.unwrap()
}

fn count(session: &mut Session<Device, Solid>, expected: &str) {
    let Reply::Evaluation { display } = call(
        session,
        Command::Eval {
            source: "count;".into(),
        },
    ) else {
        panic!()
    };
    assert_eq!(display, expected);
}

#[test]
fn stepping() {
    let (mut session, device) = setup();
    assert!(
        session
            .handle(Request::new(Command::Step {}))
            .result
            .is_err()
    );
    call(&mut session, Command::Pause {});
    call(&mut session, Command::Click { unit: 1 });
    session.tick().unwrap();
    count(&mut session, "0");
    assert!(device.0.borrow().titles.is_empty());
    assert!(matches!(
        call(&mut session, Command::Step {}),
        Reply::Stepped { dispatched: true }
    ));
    count(&mut session, "1");
    let Reply::Windows { entries } = call(&mut session, Command::Windows {}) else {
        panic!()
    };
    assert_eq!(entries[0].id, 1);
    assert_eq!(entries[0].title, "after");
    assert_eq!(entries[0].width, 640);
    assert!(matches!(
        call(&mut session, Command::Step {}),
        Reply::Stepped { dispatched: false }
    ));
    call(&mut session, Command::Click { unit: 1 });
    call(&mut session, Command::Resume {});
    session.tick().unwrap();
    count(&mut session, "2");
}

#[test]
fn closing() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    call(&mut session, Command::Click { unit: 1 });
    call(&mut session, Command::Close { unit: 1 });
    assert!(device.0.borrow().windows.is_empty());
    assert!(
        session
            .handle(Request::new(Command::Click { unit: 1 }))
            .result
            .is_err()
    );
    session.tick().unwrap();
    count(&mut session, "0");
    call(&mut session, Command::Step {});
    count(&mut session, "100");
    call(&mut session, Command::Resume {});
    assert!(!session.active());
}

#[test]
fn native() {
    let (mut session, device) = setup();
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    call(&mut session, Command::Pause {});
    device.0.borrow_mut().signals.push(device::close(unit));
    session.tick().unwrap();
    assert!(device.0.borrow().windows.is_empty());
    count(&mut session, "0");
    call(&mut session, Command::Step {});
    count(&mut session, "100");
}

#[test]
fn rejection() {
    let (mut session, _) = setup();
    assert!(
        session
            .handle(Request {
                version: 2,
                command: Command::Shutdown {}
            })
            .result
            .is_err()
    );
    assert!(session.active());
    assert!(
        session
            .handle(Request::new(Command::Close { unit: 99 }))
            .result
            .is_err()
    );
    assert!(
        session
            .handle(Request::new(Command::Eval {
                source: "missing();".into()
            }))
            .result
            .is_err()
    );
    count(&mut session, "0");
    assert!(matches!(
        call(&mut session, Command::Status {}),
        Reply::Status {
            pid: 42,
            mode: Mode::Running,
            windows: 1,
            ..
        }
    ));
}

#[test]
fn fault() {
    let (mut session, _) = setup();
    call(
        &mut session,
        Command::Eval {
            source: "win.on('click', function () { missing(); });".into(),
        },
    );
    call(&mut session, Command::Click { unit: 1 });
    session.tick().unwrap();
    assert!(matches!(
        call(&mut session, Command::Status {}),
        Reply::Status {
            mode: Mode::Paused,
            fault: Some(_),
            ..
        }
    ));
    count(&mut session, "1");
    call(&mut session, Command::Shutdown {});
    assert!(!session.active());
}

#[test]
fn shutdown() {
    let (mut session, device) = setup();
    call(
        &mut session,
        Command::Eval {
            source: "win.on('close', function () { platform.window({}); });".into(),
        },
    );
    call(&mut session, Command::Pause {});
    call(&mut session, Command::Click { unit: 1 });
    let Reply::Shutdown { errors } = call(&mut session, Command::Shutdown {}) else {
        panic!()
    };
    assert!(errors.is_empty());
    assert!(device.0.borrow().windows.is_empty());
    assert_eq!(device.0.borrow().closed.len(), 1);
    assert!(!session.active());
    assert!(
        session
            .handle(Request::new(Command::Resume {}))
            .result
            .is_err()
    );
}

#[test]
fn pressure() {
    let (mut session, device) = setup();
    call(&mut session, Command::Pause {});
    for _ in 0..256 {
        call(&mut session, Command::Click { unit: 1 });
    }
    assert!(
        session
            .handle(Request::new(Command::Click { unit: 1 }))
            .result
            .is_err()
    );
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.extend(device::click(unit));
    session.tick().unwrap();
    assert!(matches!(
        call(&mut session, Command::Status {}),
        Reply::Status {
            queued: 256,
            dropped: 1,
            ..
        }
    ));
    call(&mut session, Command::Close { unit: 1 });
    session.tick().unwrap();
    assert!(matches!(
        call(&mut session, Command::Status {}),
        Reply::Status { queued: 1, .. }
    ));
}

mod control;

mod config;
