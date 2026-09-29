use spec::scenario::{self, Options};

fn options() -> Options<'static> {
    Options {
        size: "small",
        mode: "--check",
        backend: "cpu",
        platform: (std::env::consts::ARCH, std::env::consts::OS),
    }
}

#[test]
fn lifecycle() {
    let report = scenario::run("chat", options()).unwrap();
    let run = &report.runs[0];
    assert!(run.closed);
    assert!(run.failure.is_none());
    assert!(
        run.stages
            .iter()
            .all(|stage| stage.computation.projections == 0)
    );
    let stage = |action| {
        run.stages
            .iter()
            .find(|stage| stage.action == action)
            .unwrap()
    };
    assert!(stage("seed").reuse.as_ref().unwrap().estimation.error > 0);
    assert!(stage("edit").relevance.stale > 0);
    assert_eq!(stage("reorder").relevance.stale, 0);
    assert_eq!(stage("scroll").computation.layouts, 0);
    let received = stage("receive");
    assert!(received.bindings.appends > 0);
    assert!(received.computation.layouts > 0);
    let resumed = stage("resume");
    assert!(resumed.computation.layouts > 0);
    let switched = stage("switch");
    assert!(switched.bindings.removes > 0);
    assert!(switched.bindings.appends > switched.records as u64);
    assert!(switched.computation.layouts > 0);
}

#[test]
fn convergence() {
    let config = config::Store::default();
    config.set("layout.convergence", "deferred".into());
    let report = scenario::configured("chat", options(), config).unwrap();
    let run = &report.runs[0];
    assert!(run.closed);
    assert!(run.failure.is_none());
    for stage in &run.stages {
        assert_eq!(stage.computation.projections, stage.computation.layouts);
        assert_eq!(stage.computation.finalizations, stage.computation.paints);
    }
    let resume = run
        .stages
        .iter()
        .find(|stage| stage.action == "resume")
        .unwrap();
    assert!(resume.computation.layouts > resume.computation.paints);
}
