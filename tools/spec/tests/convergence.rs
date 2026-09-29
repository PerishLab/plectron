use spec::scenario::{self, Options};

fn run(mode: &str) -> scenario::Report {
    let config = config::Store::default();
    config.set("layout.convergence", mode.into());
    scenario::configured(
        "convergence",
        Options {
            size: "small",
            mode: "--check",
            backend: "cpu",
            platform: (std::env::consts::ARCH, std::env::consts::OS),
        },
        config,
    )
    .unwrap()
}

#[test]
fn batching() {
    let eager = run("eager");
    let adaptive = run("adaptive");
    let deferred = run("deferred");
    let eager = &eager.runs[0];
    let adaptive = &adaptive.runs[0];
    let deferred = &deferred.runs[0];
    assert!(eager.failure.is_none());
    assert!(adaptive.failure.is_none());
    assert!(deferred.failure.is_none());
    assert_eq!(eager.stages.len(), deferred.stages.len());

    for ((eager, adaptive), deferred) in eager
        .stages
        .iter()
        .zip(&adaptive.stages)
        .zip(&deferred.stages)
    {
        assert_eq!(eager.action, deferred.action);
        assert_eq!(eager.action, adaptive.action);
        assert_eq!(eager.records, deferred.records);
        assert_eq!(eager.records, adaptive.records);
        assert_eq!(eager.geometry.boxes, deferred.geometry.boxes);
        assert_eq!(eager.geometry.boxes, adaptive.geometry.boxes);
        assert_eq!(
            eager.rendering.as_ref().map(|rendering| rendering.checksum),
            adaptive
                .rendering
                .as_ref()
                .map(|rendering| rendering.checksum)
        );
        assert_eq!(
            eager.rendering.as_ref().map(|rendering| rendering.checksum),
            deferred
                .rendering
                .as_ref()
                .map(|rendering| rendering.checksum)
        );
        assert_eq!(
            deferred.computation.projections,
            deferred.computation.layouts
        );
        assert_eq!(
            deferred.computation.finalizations,
            deferred.computation.paints
        );
    }

    let eager = &eager.stages[0];
    let adaptive = &adaptive.stages[0];
    let deferred = &deferred.stages[0];
    assert_eq!(
        (eager.computation.layouts, eager.computation.paints),
        (5, 5)
    );
    assert_eq!(
        (
            adaptive.computation.layouts,
            adaptive.computation.projections,
            adaptive.computation.paints,
        ),
        (5, 3, 3)
    );
    assert_eq!(
        (
            deferred.computation.layouts,
            deferred.computation.projections,
            deferred.computation.paints,
        ),
        (5, 5, 1)
    );
}

#[test]
fn visual() {
    let report = spec::benchmark::composition::visual::run(("test", "test"), 32).unwrap();
    let value = serde_json::to_value(report).unwrap();
    let samples = value["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 7);
    for sample in samples {
        assert_eq!(sample["initial"]["logical"], 100000);
        assert!(
            sample["initial"]["live"]
                .as_u64()
                .is_some_and(|live| live < 64)
        );
        assert!(
            sample["scroll"]["computation"]["layouts"]
                .as_u64()
                .is_some_and(|layouts| layouts <= 8)
        );
        assert!(
            sample["scroll"]["state"]["start"]
                .as_u64()
                .is_some_and(|start| start > 0)
        );
    }
}
