use super::{Debugger, job, setup};
use crate::{device::Device, web::call};
use plectron::{
    addons::control::Control,
    control::{Phase, proxy::Proxy},
    ipc::{Command, Request, Session},
    platform::Platform,
    rendering::Solid,
    runtime::Runtime,
};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn pending() {
    let _guard = super::serial();
    let device = Device::default();
    let platform = Platform::threaded(device.clone(), Solid);
    let source = fixtures::script("platform, console", "platform.window({});");
    let mut session = Session::spawn(platform, &source, (42, false), None).unwrap();
    session.tick().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !session.inspect().waiting {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let start = Instant::now();
    call(&mut session, Command::Shutdown {});
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(!session.active());
    session.tick().unwrap();
    assert!(device.0.borrow().windows.is_empty());
    assert!(
        session
            .handle(Request::new(Command::Eval {
                source: "platform.window({});".into()
            }))
            .result
            .is_err()
    );
}
#[test]
fn disconnected() {
    let _guard = super::serial();
    let (proxy, port) = Proxy::channel();
    let (sender, receiver) = mpsc::sync_channel(1);
    let thread = std::thread::spawn(move || {
        let mut runtime = Runtime::new(Control::new(proxy)).unwrap();
        let result = runtime
            .execute(&fixtures::script(
                "platform, console",
                "platform.window({});",
            ))
            .map(|value| value.text());
        drop(runtime);
        sender.send(result).unwrap();
    });
    let request = port.recv_timeout(Duration::from_secs(5)).unwrap();
    drop(request);
    drop(port);
    let result = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(result.unwrap_err().to_string().contains("disconnected"));
    thread.join().unwrap();
}
#[test]
fn stale() {
    let _guard = super::serial();
    let (mut session, device) = setup(
        "let closed = 0; const win = platform.window({}); win.on('close', function () { closed = closed + 1; });",
    );
    let held = job(call(&mut session, Command::Hold { unit: None }));
    session.until(|session| session.phase(held) == Phase::Held);
    let queued = job(call(
        &mut session,
        Command::Eval {
            source: "win.title('late');".into(),
        },
    ));
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device
        .0
        .borrow_mut()
        .signals
        .push(crate::device::close(unit));
    session.tick().unwrap();
    assert!(device.0.borrow().windows.is_empty());
    assert_eq!(session.phase(queued), Phase::Queued);
    call(&mut session, Command::Unhold { unit: None });
    assert!(session.complete(queued).unwrap_err().contains("closed"));
    session.until(|session| {
        session.inspect().jobs.iter().any(|job| {
            job.kind == plectron::control::Kind::Dispatch && job.phase() == Phase::Complete
        })
    });
    assert_eq!(session.evaluate("closed;").unwrap(), "1");
    assert!(device.0.borrow().titles.is_empty());
    call(&mut session, Command::Shutdown {});
}
#[test]
fn held() {
    let _guard = super::serial();
    let root = fixtures::root().join("spaces");
    let source = format!(
        "const server = platform.serve({:?}); const win = platform.window({{}}); win.on('close', function () {{ console.log('must not gate shutdown'); platform.window({{}}); }});",
        root.to_str().unwrap()
    );
    let (mut session, device) = setup(&source);
    let plectron::ipc::Reply::Services { entries } = call(&mut session, Command::Services {})
    else {
        panic!()
    };
    let address = entries[0].address.strip_prefix("http://").unwrap();
    let held = job(call(&mut session, Command::Hold { unit: None }));
    session.until(|session| session.phase(held) == Phase::Held);
    call(
        &mut session,
        Command::Eval {
            source: "platform.window({});".into(),
        },
    );
    let start = Instant::now();
    call(&mut session, Command::Shutdown {});
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(device.0.borrow().windows.is_empty());
    assert_eq!(device.0.borrow().closed.len(), 1);
    assert!(device.0.borrow().logs.is_empty());
    assert!(std::net::TcpStream::connect(address).is_err());
}
