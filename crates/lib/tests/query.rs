use plectron::ipc::query::{Command, Production, Record, Service, State, Status, Store, Target};
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

fn target(job: u64) -> Target {
    Target::Task { unit: None, job }
}
fn record(reply: plectron::ipc::query::Reply) -> Record {
    let Status::Record { record } = reply.status else {
        panic!("expected retained record")
    };
    record
}
#[test]
fn history() {
    let store = Store::new(1);
    let first = store.admit(target(50)).unwrap();
    let old = record(store.read(&Command::Read { record: first }));
    let produced = Instant::now();
    let visible = Instant::now();
    store.finish(
        first,
        State::Complete {
            result: Ok("done".into()),
        },
        Production {
            produced: Some(produced),
            visible: Some(visible),
            drops: 2,
        },
    );
    let finished = record(store.read(&Command::Read { record: first }));
    assert_eq!(old.state, State::Pending);
    assert!(finished.admitted <= finished.produced.unwrap());
    assert!(finished.produced.unwrap() <= finished.visible.unwrap());
    assert!(finished.visible.unwrap() <= finished.seen.unwrap());
    assert!(finished.seen.unwrap() <= finished.detected);
    assert_eq!(finished.drops, 2);
    assert!(finished.version > old.version);
    let second = store.admit(target(500)).unwrap();
    store.finish(
        second,
        State::Complete {
            result: Err("failed".into()),
        },
        None,
    );
    assert!(matches!(
        store.read(&Command::Read { record: first }).status,
        Status::Evicted { .. }
    ));
    assert!(matches!(
        store.read(&Command::Read { record: second + 1 }).status,
        Status::Unknown { .. }
    ));
    assert!(matches!(
        store.read(&Command::Read { record: 0 }).status,
        Status::Unknown { .. }
    ));
    assert_eq!(
        record(store.read(&Command::Read { record: second })).produced,
        None
    );
}
#[test]
fn latest() {
    let store = Store::new(0);
    let id = store
        .admit(Target::Frame {
            unit: 3,
            frame: 8,
            scene: 13,
        })
        .unwrap();
    store.finish(
        id,
        State::Complete {
            result: Ok(String::new()),
        },
        None,
    );
    let completed = record(store.read(&Command::Latest { unit: 3 }));
    let visible = Instant::now();
    store.visible(id, visible, 1);
    let presented = record(store.read(&Command::Latest { unit: 3 }));
    assert_eq!(presented.id, id);
    assert_eq!(presented.detected, completed.detected);
    assert!(presented.visible.unwrap() <= presented.seen.unwrap());
    assert_eq!(presented.drops, 1);
    assert!(presented.version > completed.version);
    assert!(matches!(
        store.read(&Command::Read { record: id }).status,
        Status::Evicted { .. }
    ));
    store.cancel(Some(3));
    assert!(matches!(
        store.read(&Command::Latest { unit: 3 }).status,
        Status::Absent { .. }
    ));
}
#[test]
fn notification() {
    let store = Store::default();
    let service = Arc::new(Service::new(store.reader()).unwrap());
    let id = store.admit(target(1)).unwrap();
    let (ready, started) = mpsc::channel();
    let threads: Vec<_> = (0..4)
        .map(|_| {
            let service = service.clone();
            let ready = ready.clone();
            std::thread::spawn(move || {
                ready.send(()).unwrap();
                service
                    .ask(Command::Wait {
                        record: id,
                        timeout: 1000,
                    })
                    .unwrap()
            })
        })
        .collect();
    for _ in 0..4 {
        started.recv_timeout(Duration::from_secs(1)).unwrap();
    }
    store.finish(
        id,
        State::Complete {
            result: Ok("done".into()),
        },
        Some(Instant::now()),
    );
    for thread in threads {
        let reply = thread.join().unwrap();
        assert!(!reply.timedout);
        assert!(matches!(record(reply).state, State::Complete { .. }));
    }
    let before = record(service.ask(Command::Read { record: id }).unwrap());
    let after = record(
        service
            .ask(Command::Wait {
                record: id,
                timeout: 1000,
            })
            .unwrap(),
    );
    assert_eq!(before.version, after.version);
    assert_eq!(before.detected, after.detected);
}
#[test]
fn termination() {
    let store = Store::default();
    let service = Service::new(store.reader()).unwrap();
    assert!(Service::new(store.reader()).is_err());
    let id = store.admit(target(1)).unwrap();
    let reply = service
        .ask(Command::Wait {
            record: id,
            timeout: 0,
        })
        .unwrap();
    assert!(reply.timedout);
    assert_eq!(record(reply).state, State::Pending);
    assert!(
        service
            .ask(Command::Wait {
                record: id,
                timeout: 3000
            })
            .is_err()
    );
    store.cancel(None);
    let reply = service
        .ask(Command::Wait {
            record: id,
            timeout: 1000,
        })
        .unwrap();
    assert!(reply.closed);
    assert!(!reply.timedout);
    assert!(matches!(record(reply).state, State::Cancelled { .. }));
    assert!(store.admit(target(2)).is_err());
}

#[test]
fn ownership() {
    let store = Store::default();
    let id = store.admit(target(1)).unwrap();
    let service = Service::new(store.reader()).unwrap();
    drop(service);
    assert_eq!(
        record(store.read(&Command::Read { record: id })).state,
        State::Pending
    );
    let service = Service::new(store.reader()).unwrap();
    store.finish(
        id,
        State::Complete {
            result: Ok(String::new()),
        },
        None,
    );
    assert!(matches!(
        record(service.ask(Command::Read { record: id }).unwrap()).state,
        State::Complete { .. }
    ));
}
