use super::{call, device::Device};
use ::config::{Command as Setting, Value};
use plectron::{
    addons::control::Control,
    ipc::{Command, Reply, Request, Session},
    platform::Platform,
    rendering::Solid,
    runtime::Runtime,
};
use std::time::{Duration, Instant};

fn setup(threaded: bool) -> Session<Device, Solid> {
    let platform = if threaded {
        Platform::threaded(Device::default(), Solid)
    } else {
        Platform::new(Device::default(), Solid)
    };
    let runtime = Runtime::new(Control::new(platform)).unwrap();
    let mut session = Session::new(runtime, 42, false);
    setting(&mut session, 0.into());
    let root = fixtures::root();
    let source = crate::application("apps/spaces/src/start.ts")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    call(&mut session, Command::Eval { source });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        session.tick().unwrap();
        if crate::web::snapshot(&mut session).state == webengine::State::Ready {
            evaluate(&mut session, &fixtures::script("events", "")).unwrap();
            return session;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn setting(session: &mut Session<Device, Solid>, value: Value) {
    let Reply::Config { entries } = call(
        session,
        Command::Config {
            command: Setting::Set {
                key: "events.scopes".into(),
                value: value.clone(),
            },
        },
    ) else {
        panic!()
    };
    assert_eq!(entries["events.scopes"], value);
}

fn evaluate(session: &mut Session<Device, Solid>, source: &str) -> Result<String, String> {
    let result = session
        .handle(Request::new(Command::Evaluate {
            unit: 1,
            source: source.into(),
        }))
        .result?;
    let job = match result {
        Reply::Evaluation { display } => return Ok(display),
        Reply::Accepted { job, .. } => job,
        _ => panic!(),
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        session.tick().unwrap();
        let snapshot = crate::web::snapshot(session);
        let execution = snapshot.execution.unwrap();
        if let Some(result) = execution
            .jobs
            .iter()
            .find(|entry| entry.id == job)
            .unwrap()
            .result()
        {
            return result.clone();
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn distribution() {
    for threaded in [false, true] {
        let mut session = setup(threaded);
        assert!(
            evaluate(&mut session, "events.scope();")
                .unwrap_err()
                .contains("scope budget")
        );
        setting(&mut session, 1.into());
        evaluate(&mut session, "const scope = events.scope();").unwrap();
        setting(&mut session, (-1).into());
        assert!(
            evaluate(&mut session, "events.scope();")
                .unwrap_err()
                .contains("events.scopes")
        );
        evaluate(&mut session, "scope.dispose();").unwrap();
        setting(&mut session, 1.into());
        evaluate(&mut session, "events.scope();").unwrap();
        call(&mut session, Command::Shutdown {});
    }
}
