use crate::{
    device::Device,
    web::{call, snapshot},
};
use plectron::{
    addons::control::Control,
    ipc::{Command, Reply, Session},
    platform::{
        Host, Platform,
        device::{Event, Point, Signal},
    },
    rendering::Solid,
    runtime::Runtime,
};
use std::time::{Duration, Instant};
use webengine::{Phase, Snapshot};

fn setup() -> (Session<Device, Solid>, Device) {
    let device = Device::default();
    let platform = Platform::threaded(device.clone(), Solid);
    let mut runtime = Runtime::new(Control::new(platform)).unwrap();
    let root = fixtures::root();
    let source = crate::application("apps/spaces/start.ts")
        .replace("\"apps\"", &format!("{:?}", root.to_str().unwrap()));
    runtime.execute(&source).unwrap();
    runtime.addons().host.finish().unwrap();
    let mut session = Session::new(runtime, 42, false);
    until(&mut session, |snapshot| snapshot.frame.is_some());
    (session, device)
}
fn until(session: &mut Session<Device, Solid>, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        session.tick().unwrap();
        if predicate(&snapshot(session)) {
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn job(reply: Reply) -> u64 {
    let Reply::Accepted { job, .. } = reply else {
        panic!()
    };
    job
}
fn phase(snapshot: &Snapshot, id: u64) -> Phase {
    snapshot
        .execution
        .as_ref()
        .unwrap()
        .jobs
        .iter()
        .find(|job| job.id == id)
        .unwrap()
        .phase()
}
#[test]
fn independent() {
    let (mut session, device) = setup();
    let held = job(call(&mut session, Command::Hold { unit: Some(1) }));
    until(&mut session, |snapshot| {
        phase(snapshot, held) == Phase::Held
    });
    let before = snapshot(&mut session);
    let queued = job(call(
        &mut session,
        Command::Evaluate {
            unit: 1,
            source: "document.getElementById('status').setText('worker resumed');".into(),
        },
    ));
    call(&mut session, Command::Scroll { unit: 1, delta: 24 });
    let scrolled = snapshot(&mut session);
    assert_eq!(scrolled.scene, before.scene);
    assert_eq!(scrolled.shown.unwrap().offset(0), 24);
    assert_ne!(scrolled.frame, before.frame);
    let unit = *device.0.borrow().windows.keys().next().unwrap();
    device.0.borrow_mut().signals.push(Signal {
        unit,
        event: Event::Wheel {
            position: Point { x: 40, y: 110 },
            delta: 16,
        },
    });
    until(&mut session, |snapshot| {
        snapshot.visual.revision > scrolled.visual.revision
    });
    assert_eq!(phase(&snapshot(&mut session), queued), Phase::Queued);
    call(&mut session, Command::Unhold { unit: Some(1) });
    until(&mut session, |snapshot| {
        phase(snapshot, queued) == Phase::Complete
    });
    let complete = snapshot(&mut session);
    assert!(
        complete
            .nodes
            .iter()
            .any(|node| node.content == "worker resumed")
    );
    assert_eq!(complete.visual.offset(0), 24);
    let held = job(call(&mut session, Command::Hold { unit: Some(1) }));
    until(&mut session, |snapshot| {
        phase(snapshot, held) == Phase::Held
    });
    let start = Instant::now();
    call(&mut session, Command::Close { unit: 1 });
    assert!(start.elapsed() < Duration::from_secs(1));
    assert!(device.0.borrow().windows.is_empty());
    call(&mut session, Command::Shutdown {});
}
