mod isolation;
mod performance;
mod shutdown;
mod source;
use crate::{device::Device, web::call};
use plectron::{
    control::{Phase, Snapshot},
    ipc::{Command, Reply, Session},
    platform::Platform,
    rendering::Solid,
};
use std::time::{Duration, Instant};

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

fn setup(source: &str) -> (Session<Device, Solid>, Device) {
    start(&fixtures::script("platform, console", source))
}

fn start(source: &str) -> (Session<Device, Solid>, Device) {
    let device = Device::default();
    let platform = Platform::threaded(device.clone(), Solid);
    let mut session = Session::spawn(platform, source, (42, false), None).unwrap();
    session.until(|session| session.inspect().jobs[0].phase() == Phase::Complete);
    (session, device)
}
fn job(reply: Reply) -> u64 {
    let Reply::Accepted { job, .. } = reply else {
        panic!()
    };
    job
}
trait Debugger {
    fn inspect(&mut self) -> Snapshot;
    #[track_caller]
    fn until(&mut self, predicate: impl Fn(&mut Session<Device, Solid>) -> bool);
    fn phase(&mut self, id: u64) -> Phase;
    fn complete(&mut self, id: u64) -> Result<String, String>;
    fn evaluate(&mut self, source: &str) -> Result<String, String>;
}
impl Debugger for Session<Device, Solid> {
    fn inspect(&mut self) -> Snapshot {
        let Reply::Control { snapshot } = call(self, Command::Control {}) else {
            panic!()
        };
        snapshot
    }
    fn until(&mut self, predicate: impl Fn(&mut Session<Device, Solid>) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            self.tick().unwrap();
            if predicate(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "control progress timed out: {:?}",
                call(self, Command::Status {})
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    fn phase(&mut self, id: u64) -> Phase {
        self.inspect()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .phase()
    }
    fn complete(&mut self, id: u64) -> Result<String, String> {
        self.until(|session| session.phase(id) == Phase::Complete);
        self.inspect()
            .jobs
            .into_iter()
            .find(|job| job.id == id)
            .unwrap()
            .result()
            .cloned()
            .unwrap()
    }
    fn evaluate(&mut self, source: &str) -> Result<String, String> {
        let id = job(call(
            self,
            Command::Eval {
                source: source.into(),
            },
        ));
        self.complete(id)
    }
}

#[test]
fn lifecycle() {
    let _guard = serial();
    let (mut session, device) = setup(
        "const first = platform.window({}); first.fill(1); first.fill(3); const second = platform.window({}); second.fill(2);",
    );
    let record = device.0.borrow();
    assert_eq!(record.prepared, 1);
    assert_eq!(record.activated.len(), 2);
    drop(record);
    call(&mut session, Command::Shutdown {});
}

#[test]
fn preparation() {
    let _guard = serial();
    let device = Device::default();
    device.0.borrow_mut().reject = Some("prepare");
    let platform = Platform::threaded(device.clone(), Solid);
    let result = Session::spawn(
        platform,
        &fixtures::script("platform, console", "platform.window({});"),
        (42, false),
        None,
    );
    assert!(result.is_err());
    let record = device.0.borrow();
    assert_eq!(record.prepared, 1);
    assert!(record.windows.is_empty());
}

#[test]
fn stepping() {
    let _guard = serial();
    let (mut session, device) = setup(
        "let count = 0; const win = platform.window({}); win.on('click', function () { count = count + 1; win.title('clicked'); });",
    );
    call(&mut session, Command::Pause {});
    call(&mut session, Command::Click { unit: 1 });
    session.tick().unwrap();
    assert_eq!(session.evaluate("count;").unwrap(), "0");
    assert!(matches!(
        call(&mut session, Command::Step {}),
        Reply::Stepped { dispatched: true }
    ));
    session.until(|_| !device.0.borrow().titles.is_empty());
    assert_eq!(session.evaluate("count;").unwrap(), "1");
    session
        .evaluate("win.on('click', function () { missing(); });")
        .unwrap();
    call(&mut session, Command::Click { unit: 1 });
    call(&mut session, Command::Resume {});
    session.until(|session| {
        matches!(
            call(session, Command::Status {}),
            Reply::Status {
                mode: plectron::ipc::Mode::Paused,
                fault: Some(_),
                ..
            }
        )
    });
    assert_eq!(session.evaluate("count;").unwrap(), "2");
    call(&mut session, Command::Shutdown {});
}
#[test]
fn pressure() {
    let _guard = serial();
    let (mut session, _) = setup("let count = 0;");
    let held = job(call(&mut session, Command::Hold { unit: None }));
    session.until(|session| session.phase(held) == Phase::Held);
    assert!(
        session
            .handle(plectron::ipc::Request::new(Command::Hold { unit: None }))
            .result
            .is_err()
    );
    let mut ids = Vec::new();
    for _ in 0..8 {
        ids.push(job(call(
            &mut session,
            Command::Eval {
                source: "count = count + 1; count;".into(),
            },
        )));
    }
    assert!(
        session
            .handle(plectron::ipc::Request::new(Command::Eval {
                source: "count = 999;".into()
            }))
            .result
            .is_err()
    );
    call(&mut session, Command::Unhold { unit: None });
    assert!(
        session
            .handle(plectron::ipc::Request::new(Command::Hold { unit: None }))
            .result
            .is_err()
    );
    for (index, id) in ids.into_iter().enumerate() {
        assert_eq!(session.complete(id).unwrap(), (index + 1).to_string());
    }
    assert_eq!(session.evaluate("count;").unwrap(), "8");
    let finalhold = job(call(&mut session, Command::Hold { unit: None }));
    session.until(|session| session.phase(finalhold) == Phase::Held);
    call(&mut session, Command::Shutdown {});
}

#[test]
fn generations() {
    let _guard = serial();
    let (mut session, _) = setup("0;");
    for _ in 0..4 {
        call(
            &mut session,
            Command::Eval {
                source: "0;".into(),
            },
        );
    }
    let first = job(call(&mut session, Command::Hold { unit: None }));
    call(&mut session, Command::Unhold { unit: None });
    let middle = job(call(
        &mut session,
        Command::Eval {
            source: "42;".into(),
        },
    ));
    let second = job(call(&mut session, Command::Hold { unit: None }));
    session.until(|session| session.phase(second) == Phase::Held);
    assert_eq!(session.phase(first), Phase::Complete);
    assert_eq!(session.phase(middle), Phase::Complete);
    call(&mut session, Command::Shutdown {});
}

#[test]
fn history() {
    let _guard = serial();
    let (mut session, _) = setup("let count = 0;");
    for count in 1..=70 {
        assert_eq!(
            session.evaluate("count = count + 1; count;").unwrap(),
            count.to_string()
        );
    }
    let snapshot = session.inspect();
    assert_eq!(snapshot.jobs.len(), 64);
    assert!(snapshot.jobs.iter().all(|job| {
        job.kind == plectron::control::Kind::Evaluate && job.phase() == Phase::Complete
    }));
    session.tick().unwrap();
    call(&mut session, Command::Shutdown {});
}
