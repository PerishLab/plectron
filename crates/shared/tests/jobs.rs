use shared::jobs::{Fault, Jobs, Phase, State};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Evaluate,
    Hold,
}
#[test]
fn transitions() {
    let mut jobs = Jobs::new(64);
    jobs.admit(1, Kind::Evaluate).unwrap();
    assert_eq!(jobs.hold(1), Err(Fault::Transition));
    assert_eq!(jobs.finish(1, Ok("early".into())), Err(Fault::Transition));
    assert!(jobs.start(1).unwrap());
    assert_eq!(jobs.start(1), Err(Fault::Transition));
    assert!(jobs.finish(1, Err("script failed".into())).unwrap());
    assert!(!jobs.finish(1, Ok("late".into())).unwrap());
    assert!(!jobs.start(1).unwrap());
    assert!(!jobs.hold(1).unwrap());
    jobs.close("shutdown");
    assert_eq!(
        jobs.get(1).unwrap().result(),
        Some(&Err("script failed".into()))
    );
    assert_eq!(jobs.admit(2, Kind::Hold), Err(Fault::Closed));
}
#[test]
fn cancellation() {
    let mut jobs = Jobs::new(64);
    for id in 1..=3 {
        jobs.admit(id, Kind::Hold).unwrap();
    }
    jobs.start(2).unwrap();
    jobs.start(3).unwrap();
    jobs.hold(3).unwrap();
    jobs.close("shutdown");
    for id in 1..=3 {
        assert_eq!(jobs.get(id).unwrap().phase(), Phase::Cancelled);
        assert!(!jobs.finish(id, Ok("late".into())).unwrap());
        assert!(!jobs.start(id).unwrap());
    }
    jobs.close("different reason");
    assert!(
        jobs.snapshot()
            .iter()
            .all(|job| matches!(&job.state, State::Cancelled { reason } if reason == "shutdown"))
    );
}
#[test]
fn history() {
    let mut jobs = Jobs::new(2);
    jobs.admit(1, Kind::Hold).unwrap();
    jobs.start(1).unwrap();
    jobs.hold(1).unwrap();
    jobs.admit(2, Kind::Evaluate).unwrap();
    jobs.start(2).unwrap();
    jobs.finish(2, Ok("2".into())).unwrap();
    jobs.admit(3, Kind::Evaluate).unwrap();
    assert!(jobs.get(2).is_none());
    assert_eq!(jobs.get(1).unwrap().phase(), Phase::Held);
    assert_eq!(jobs.admit(2, Kind::Evaluate), Err(Fault::Identifier));
    jobs.admit(4, Kind::Evaluate).unwrap();
    assert_eq!(jobs.snapshot().len(), 3);
    jobs.close("shutdown");
    assert_eq!(jobs.snapshot().len(), 2);
    assert!(jobs.get(1).is_none());
    assert_eq!(jobs.start(99), Err(Fault::Unknown));
}
#[test]
fn ownership() {
    let mut first = Jobs::new(64);
    let mut second = Jobs::new(64);
    first.admit(1, Kind::Evaluate).unwrap();
    second.admit(1, Kind::Hold).unwrap();
    let mut snapshot = first.snapshot();
    snapshot[0].state = State::Cancelled {
        reason: "external".into(),
    };
    assert_eq!(first.get(1).unwrap().phase(), Phase::Queued);
    first.close("closed");
    assert!(second.start(1).unwrap());
    assert_eq!(second.get(1).unwrap().kind, Kind::Hold);
}
