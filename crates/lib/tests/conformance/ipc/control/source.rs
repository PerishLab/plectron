use super::{Debugger, serial};
use crate::{device::Device, web::call};
use plectron::{
    ipc::{Command, Session},
    platform::Platform,
    rendering::Solid,
};

#[test]
fn callbacks() {
    let _guard = serial();
    let entry = project::Module::new("entry.ts").unwrap();
    let text = "const saved = function (): never { throw 'retained'; };";
    let output = project::Linker::new(
        project::Memory::new([(entry.clone(), text.into())]),
        project::Resolver::new([]).unwrap(),
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .link(entry, project::Target::Script)
    .unwrap();
    let script = plectron::runtime::source::script(output);
    let device = Device::default();
    let platform = Platform::threaded(device, Solid);
    let mut session = Session::spawn(platform, script, (42, false), None).unwrap();
    session
        .until(|session| session.inspect().jobs[0].phase() == plectron::control::Phase::Complete);
    assert!(session.inspect().jobs[0].result().unwrap().is_ok());
    session.evaluate("const unrelated = 1;").unwrap();
    let failure = session.evaluate("saved();").unwrap_err();
    let start = text.find("throw").unwrap();
    let end = start + "throw 'retained';".len();
    assert_eq!(
        failure,
        format!("entry.ts:{start}..{end}: uncaught exception: retained")
    );
    call(&mut session, Command::Shutdown {});
}

#[test]
fn wake() {
    let _guard = serial();
    let (sender, receiver) = std::sync::mpsc::channel();
    let notify = shared::Notify::new(move || {
        let _ = sender.send(());
    });
    let platform = Platform::threaded(Device::default(), Solid);
    let mut session = Session::spawn(platform, "0;", (42, false), Some(notify)).unwrap();
    session
        .until(|session| session.inspect().jobs[0].phase() == plectron::control::Phase::Complete);
    receiver.try_iter().for_each(drop);
    let id = super::job(call(
        &mut session,
        Command::Eval {
            source: "2 + 3;".into(),
        },
    ));
    receiver
        .recv_timeout(std::time::Duration::from_secs(1))
        .expect("new control work must publish without an extra owner tick");
    assert_eq!(session.complete(id).unwrap(), "5");
    call(&mut session, Command::Shutdown {});
}

#[test]
fn faulted() {
    let _guard = serial();
    for manual in [false, true] {
        let (mut session, device) = super::setup(
            "const win = platform.window({}); win.on('click', function () { missing(); }); win.on('close', function () { console.log('closed'); });",
        );
        let unit = *device.0.borrow().windows.keys().next().unwrap();
        call(&mut session, Command::Click { unit: 1 });
        session.until(|session| {
            matches!(
                call(session, Command::Status {}),
                plectron::ipc::Reply::Status {
                    mode: plectron::ipc::Mode::Paused,
                    fault: Some(_),
                    ..
                }
            )
        });
        if manual {
            call(&mut session, Command::Pause {});
        }
        device
            .0
            .borrow_mut()
            .signals
            .push(crate::device::close(unit));
        if manual {
            session.tick().unwrap();
            assert!(device.0.borrow().logs.is_empty());
            assert!(session.active());
            call(&mut session, Command::Step {});
            session.until(|_| !device.0.borrow().logs.is_empty());
            assert!(session.active());
            call(&mut session, Command::Resume {});
        }
        session.until(|session| !session.active());
        assert_eq!(device.0.borrow().logs, ["closed"]);
        assert_eq!(device.0.borrow().closed, [unit]);
        call(&mut session, Command::Shutdown {});
    }
}
#[test]
fn burst() {
    let _guard = serial();
    let root = fixtures::root();
    let source = crate::application("apps/todo/src/start.ts")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    let (mut session, device) = super::start(&source);
    session.until(|session| crate::web::snapshot(session).state == webengine::State::Ready);
    let prepared = super::job(call(
        &mut session,
        Command::Evaluate {
            unit: 1,
            source: "const probe = document.createElement('input'); probe.setAttribute('style', 'font-family:toy;width:160px;height:24px'); const host = document.getElementById('app'); for (const child of host.children()) { host.removeChild(child); } host.appendChild(probe); probe.focus();".into(),
        },
    ));
    session.until(|session| {
        crate::web::snapshot(session)
            .execution
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == prepared && job.phase() == webengine::Phase::Complete)
    });
    let prepared = crate::web::snapshot(&mut session)
        .execution
        .unwrap()
        .jobs
        .into_iter()
        .find(|job| job.id == prepared)
        .unwrap();
    assert!(
        prepared.result().unwrap().is_ok(),
        "{:?}",
        prepared.result()
    );
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    let hold = call(&mut session, Command::Hold { unit: Some(1) });
    let plectron::ipc::Reply::Accepted { job: hold, .. } = hold else {
        panic!()
    };
    session.until(|session| {
        crate::web::snapshot(session)
            .execution
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == hold && job.phase() == webengine::Phase::Held)
    });
    device
        .0
        .borrow_mut()
        .signals
        .extend((0..32).map(|_| ::platform::shared::Signal {
            unit,
            event: ::platform::shared::Event::Input(shared::shortcut::Input::Text("x".into())),
        }));
    for _ in 0..32 {
        session.tick().unwrap();
    }
    let plectron::ipc::Reply::Status {
        mode,
        fault,
        queued,
        ..
    } = call(&mut session, Command::Status {})
    else {
        panic!()
    };
    assert_eq!(mode, plectron::ipc::Mode::Running);
    assert!(fault.is_none());
    assert!(queued > 0);
    call(&mut session, Command::Unhold { unit: Some(1) });
    session.until(|session| {
        matches!(
            call(session, Command::Status {}),
            plectron::ipc::Reply::Status { queued: 0, .. }
        ) && crate::web::snapshot(session).nodes.iter().any(|node| {
            node.editor
                .as_ref()
                .is_some_and(|editor| editor.text == "x".repeat(32))
        })
    });
    let plectron::ipc::Reply::Status { mode, fault, .. } = call(&mut session, Command::Status {})
    else {
        panic!()
    };
    assert_eq!(mode, plectron::ipc::Mode::Running);
    assert!(fault.is_none());
    call(&mut session, Command::Shutdown {});
}
