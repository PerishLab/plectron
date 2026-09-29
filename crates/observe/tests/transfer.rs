use observe::{Envelope, Kind, Session};

#[test]
fn nesting() {
    let session = Session::new(Default::default());
    let mut source = session.scope("source");
    let envelope = Envelope::new(());
    source.finish(true);
    let mut execution = session.scope("execution");
    let (_, mut received) = envelope.receive("received");
    received.finish(true);
    execution.finish(true);
    session.stop();
    let report = session.inspect();
    let received = report
        .records
        .iter()
        .find(|r| r.name == "received")
        .unwrap();
    assert_eq!(received.parent, Some(report.records[0].id));
    assert_eq!(received.enclosing, Some(report.records[2].id));
    assert_ne!(received.parent, received.enclosing);
}

#[test]
fn handoff() {
    let session = Session::new(Default::default());
    let mut source = session.scope("sender");
    let envelope = Envelope::new(42);
    source.finish(true);
    std::thread::spawn(move || {
        let (value, mut scope) = envelope.receive("receiver");
        assert_eq!(value, 42);
        scope.finish(true);
    })
    .join()
    .unwrap();
    session.stop();
    let report = session.inspect();
    let sender = &report.records[0];
    let receiver = &report.records[1];
    assert_eq!(receiver.parent, Some(sender.id));
    let Kind::Receive { captured, thread } = receiver.kind else {
        panic!("missing handoff");
    };
    assert!((sender.start..=sender.end).contains(&captured));
    assert!(captured <= receiver.start);
    assert_eq!(thread, sender.thread);
    assert_ne!(sender.thread, receiver.thread);
    assert!(receiver.cpu.is_none());
}

#[test]
fn isolation() {
    let detached = Envelope::new(1);
    let old = Session::new(Default::default());
    let mut source = old.scope("source");
    let late = Envelope::new(2);
    source.finish(true);
    old.stop();
    let new = Session::new(Default::default());
    let mut receiver = new.scope("other");
    for envelope in [detached, late] {
        let (_, mut guard) = envelope.receive("suppressed");
        assert!(observe::current().is_none());
        observe::call("child", || Ok::<_, ()>(())).unwrap();
        guard.finish(true);
    }
    receiver.finish(true);
    assert_eq!(new.status().recorded, 1);
    assert_eq!(old.status().dropped, 1);
}
