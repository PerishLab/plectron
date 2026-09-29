use observe::{Options, Outcome, Session};

#[test]
fn detachment() {
    let session = Session::new(Options::default());
    let mut root = session.scope("root");
    {
        let mut detached = observe::resume(None, "detached");
        observe::call("unrelated", || Ok::<_, ()>(())).unwrap();
        assert!(observe::current().is_none());
        detached.finish(true);
    }
    assert!(observe::current().is_some());
    root.finish(true);
    session.stop();
    assert_eq!(session.status().recorded, 1);
    assert_eq!(session.status().dropped, 0);
}

#[test]
fn nesting() {
    let session = Session::new(Options::default());
    let mut root = session.scope("root");
    let result = observe::call("failure", || Err::<(), _>("expected"));
    assert_eq!(result, Err("expected"));
    root.finish(true);
    session.stop();
    let report = session.inspect();
    assert_eq!(report.open, 0);
    assert_eq!(report.records.len(), 2);
    assert_eq!(report.records[0].parent, Some(report.records[1].id));
    assert_eq!(report.records[0].outcome, Outcome::Failure);
    assert_eq!(report.records[1].outcome, Outcome::Success);
    assert!(observe::current().is_none());
}
#[test]
fn lifecycle() {
    let session = Session::new(Options {
        limit: 2,
        ..Options::default()
    });
    let mut root = session.scope("root");
    let child = observe::scope("child");
    drop(observe::scope("dropped"));
    session.stop();
    drop(session.scope("late"));
    assert_eq!(session.inspect().open, 2);
    drop(child);
    root.finish(true);
    let report = session.inspect();
    assert_eq!(report.dropped, 2);
    assert_eq!(report.records.len(), 2);
    assert_eq!(report.records[0].outcome, Outcome::Abandoned);
}
#[test]
fn propagation() {
    let session = Session::new(Options {
        cpu: true,
        ..Options::default()
    });
    let mut root = session.scope("root");
    let context = observe::current().unwrap();
    std::thread::spawn(move || {
        assert!(observe::current().is_none());
        let mut child = context.scope("worker");
        child.finish(true);
    })
    .join()
    .unwrap();
    root.finish(true);
    let report = session.inspect();
    let child = &report.records[0];
    let root = &report.records[1];
    assert_eq!(child.parent, Some(root.id));
    assert_ne!(child.thread, root.thread);
    assert!(child.end >= child.start);
}
#[test]
fn unwind() {
    let session = Session::new(Options::default());
    let result = std::panic::catch_unwind(|| {
        let _scope = session.scope("panic");
        panic!("expected");
    });
    assert!(result.is_err());
    assert!(observe::current().is_none());
    assert_eq!(session.inspect().records[0].outcome, Outcome::Panicked);
}
#[test]
fn disabled() {
    struct Name;
    impl From<Name> for String {
        fn from(_: Name) -> Self {
            panic!("disabled name was allocated")
        }
    }
    drop(observe::scope(Name));
    assert_eq!(observe::call(Name, || Ok::<_, ()>(42)), Ok(42));
}

#[test]
fn isolation() {
    let outer = Session::new(Options::default());
    let inner = Session::new(Options {
        limit: 0,
        ..Options::default()
    });
    let mut root = outer.scope("outer");
    {
        let _blocked = inner.scope("blocked");
        assert!(observe::current().is_none());
        observe::call("hidden", || Ok::<_, ()>(())).unwrap();
    }
    assert!(observe::current().is_some());
    root.finish(true);
    assert_eq!(outer.inspect().records.len(), 1);
    assert_eq!(inner.inspect().dropped, 1);
}
