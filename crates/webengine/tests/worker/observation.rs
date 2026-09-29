use super::{Action, Duration, Instant, Phase, Worker, phase, until};

pub(super) fn ownership(worker: &mut Worker) {
    let held = worker.hold().unwrap();
    until(worker, |snapshot| phase(snapshot, held) == Phase::Held);
    let session = observe::Session::new(observe::Options::default());
    let mut root = session.scope("mutation");
    let id = worker
        .submit(Action::Evaluate(
            "document.getElementById('a').setAttribute('style', 'width: 40px; height: 32px;');"
                .into(),
        ))
        .unwrap();
    root.finish(true);
    worker.unhold().unwrap();
    until(worker, |snapshot| phase(snapshot, id) == Phase::Complete);
    let mut collection = session.scope("collection");
    let collected = worker.submit(Action::Collect).unwrap();
    collection.finish(true);
    until(worker, |snapshot| {
        phase(snapshot, collected) == Phase::Complete
    });
    let snapshot = worker.inspect();
    assert!(
        snapshot
            .execution
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .result()
            .unwrap()
            .is_ok()
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while session.status().open != 0 {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    session.stop();
    let report = session.inspect();
    let mutation = report
        .records
        .iter()
        .find(|r| r.name == "mutation")
        .unwrap();
    let execution = report
        .records
        .iter()
        .find(|r| r.name == "content.execute")
        .unwrap();
    assert_eq!(execution.parent, Some(mutation.id));
    assert_ne!(execution.thread, mutation.thread);
    assert!(execution.start >= mutation.end);
    assert!(report.records.iter().any(|r| r.name == "js.execute"));
    assert!(
        report
            .records
            .iter()
            .any(|r| r.name == "web.collect.manual")
    );
    assert!(report.records.iter().any(|r| r.name == "js.collect"));
    assert!(report.records.iter().any(|r| r.name == "dom.collect"));
    assert!(
        report
            .records
            .iter()
            .any(|r| matches!(r.kind, observe::Kind::Node { .. }))
    );
    assert_eq!(report.dropped, 0);
    let published = report
        .records
        .iter()
        .find(|r| r.name == "content.publish" && r.parent == Some(execution.id))
        .unwrap();
    let accepted = report
        .records
        .iter()
        .find(|r| r.name == "content.accept" && r.parent == Some(published.id))
        .unwrap();
    assert_ne!(published.thread, accepted.thread);
    assert!(
        report
            .records
            .iter()
            .any(|r| r.name == "content.install" && r.parent == Some(accepted.id))
    );
    assert!(
        matches!(accepted.kind, observe::Kind::Receive { captured, .. } if captured <= accepted.start)
    );
}
