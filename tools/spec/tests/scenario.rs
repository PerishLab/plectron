use spec::scenario::{self, Options};

fn run(id: &str) -> scenario::Report {
    let report = scenario::run(
        id,
        Options {
            size: "small",
            mode: "--check",
            backend: "cpu",
            platform: (std::env::consts::ARCH, std::env::consts::OS),
        },
    )
    .unwrap();
    assert_eq!(report.runs.len(), 1);
    assert!(report.summary["edit"].reference.is_none());
    let run = &report.runs[0];
    assert!(run.closed && run.failure.is_none());
    assert_eq!(run.stages[0].records, report.count);
    assert!(run.stages[1].rendering.is_none() && run.stages[1].update.is_none());
    assert_eq!(run.stages.last().unwrap().trigger, "wheel");
    assert!(run.assets.keys().any(|url| url.ends_with(".ttf")));
    assert!(run.stages.iter().all(|s| s.nodes >= s.records));
    assert!(run.stages[0].activity.natives > 0);
    assert_eq!(
        run.stages[0].activity.natives,
        run.stages[0].bindings.total()
    );
    assert!(run.stages[1].activity == jsengine::Activity::default());
    assert!(run.stages[1].bindings == webengine::Bindings::default());
    for stage in run.stages.iter().filter_map(|s| s.rendering.as_ref()) {
        assert!(stage.maximum == 0 && stage.gpu.is_none());
        assert_ne!(stage.checksum, 0);
    }
    report
}
#[test]
fn list() {
    run("list");
}
#[test]
fn isolation() {
    run("isolation");
}
#[test]
fn article() {
    run("article");
}
#[test]
fn desk() {
    run("desk");
}
#[test]
fn gallery() {
    let report = run("gallery");
    let run = &report.runs[0];
    let stage = |action| {
        run.stages
            .iter()
            .find(|stage| stage.action == action)
            .unwrap()
    };
    let checksum = |action| stage(action).rendering.as_ref().unwrap().checksum;
    assert!(run.assets.values().all(|asset| asset.requests == 1));
    assert_eq!(
        run.assets
            .keys()
            .filter(|url| url.ends_with(".png"))
            .count(),
        2
    );
    assert_eq!(stage("seed").images.fetches, 1);
    assert_eq!(
        stage("edit").images.bindings,
        stage("append").images.bindings
    );
    assert_eq!(stage("swap").images.fetches, 2);
    assert_eq!(stage("restore").images.fetches, 2);
    assert_eq!(
        stage("swap").images.decoded,
        stage("restore").images.decoded
    );
    assert_ne!(checksum("remove"), checksum("swap"));
    assert_eq!(checksum("remove"), checksum("restore"));
}
#[test]
fn cards() {
    run("cards");
}
#[test]
fn layers() {
    let report = run("layers");
    let run = &report.runs[0];
    let stage = |action| {
        run.stages
            .iter()
            .find(|stage| stage.action == action)
            .unwrap()
            .rendering
            .as_ref()
            .unwrap()
    };
    assert_ne!(stage("remove").checksum, stage("fade").checksum);
    assert_ne!(stage("fade").checksum, stage("shift").checksum);
    assert_eq!(stage("fade").groups, stage("remove").groups + 1);
}
#[test]
fn catalog() {
    let original = scenario::load().unwrap();
    let mut cases = original.clone();
    cases.push(cases[0].clone());
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original.clone();
    cases[0].capabilities.push("ambient".into());
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original.clone();
    cases[0].entry = "../Cargo.toml".into();
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original.clone();
    cases[0].entry = "desk/index.html".into();
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original.clone();
    cases[0].launcher = "desk/start.ts".into();
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original.clone();
    cases[0].actions.swap(0, 1);
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original.clone();
    cases[0].regions = vec!["secondary".into()];
    assert!(scenario::validate(&cases).is_err());
    let mut cases = original;
    cases[0].sizes.insert("small".into(), 0);
    assert!(scenario::validate(&cases).is_err());
}

#[test]
fn admission() {
    let config = config::Store::default();
    config.set("events.scopes", 8.into());
    let report = scenario::configured(
        "list",
        Options {
            size: "medium",
            mode: "--check",
            backend: "cpu",
            platform: ("test", "test"),
        },
        config,
    )
    .unwrap();
    assert_eq!(report.count, 128);
    let run = &report.runs[0];
    assert!(run.closed);
    assert!(run.stages.is_empty());
    assert!(report.summary.is_empty());
    let failure = run.failure.as_ref().unwrap();
    assert_eq!(failure.action, "seed");
    assert!(
        failure
            .reason
            .contains("subscription scope budget exhausted")
    );
    assert_eq!(failure.records, 8);
    assert_eq!(run.config["events.scopes"], 8);
}
#[test]
fn capacity() {
    let exercise = |config| {
        scenario::configured(
            "list",
            Options {
                size: "small",
                mode: "--check",
                backend: "cpu",
                platform: ("test", "test"),
            },
            config,
        )
        .unwrap()
    };
    let cached = exercise(config::Store::default());
    let config = config::Store::default();
    let policy = shared::cache::Policy {
        read: false,
        write: false,
        entries: 0,
        bytes: 0,
    };
    config.set("layout.cache", serde_json::to_value(policy).unwrap());
    config.set("layout.paragraphs", serde_json::to_value(policy).unwrap());
    config.set("layout.intrinsic", serde_json::to_value(policy).unwrap());
    let reference = exercise(config);
    let cached = &cached.runs[0];
    let reference = &reference.runs[0];
    assert!(cached.failure.is_none());
    assert!(reference.failure.is_none());
    for (a, b) in cached.stages.iter().zip(&reference.stages) {
        assert_eq!(a.records, b.records);
        assert_eq!(
            a.rendering.as_ref().map(|r| r.checksum),
            b.rendering.as_ref().map(|r| r.checksum)
        );
    }
    let a = cached
        .stages
        .iter()
        .find(|s| s.action == "edit")
        .unwrap()
        .reuse
        .as_ref()
        .unwrap();
    let b = reference
        .stages
        .iter()
        .find(|s| s.action == "edit")
        .unwrap()
        .reuse
        .as_ref()
        .unwrap();
    assert!(a.hits > b.hits);
    assert!(a.shaping.preparations < b.shaping.preparations);
}

#[test]
fn experiments() {
    let config = config::Store::default();
    for (key, value) in [
        ("scenario.count", serde_json::json!(8)),
        ("scenario.runs", serde_json::json!(2)),
        ("scenario.edits", serde_json::json!(3)),
        ("scenario.height", serde_json::json!(64)),
        ("scenario.viewport", serde_json::json!([640, 480])),
        ("scenario.reference", serde_json::json!(0)),
        ("scenario.warmups", serde_json::json!(2)),
        ("scenario.retained", serde_json::json!(true)),
    ] {
        config.set(key, value);
    }
    let report = scenario::configured("list", options(), config).unwrap();
    assert_eq!(report.count, 8);
    assert_eq!(report.edits, 3);
    assert_eq!(report.case.viewport, [640, 480]);
    assert_eq!(report.runs.len(), 2);
    assert!(report.runs.iter().all(|r| r.closed && r.failure.is_none()));
    let edits: Vec<_> = report
        .runs
        .iter()
        .map(|r| r.stages.iter().find(|s| s.action == "edit").unwrap())
        .collect();
    for edit in &edits {
        let rendering = edit.rendering.as_ref().unwrap();
        assert_eq!(edit.records, 10);
        assert_eq!(
            edit.total,
            edit.evaluation + edit.publication + rendering.preparation + rendering.completion
        );
    }
    assert_eq!(
        edits[0].rendering.as_ref().unwrap().checksum,
        edits[1].rendering.as_ref().unwrap().checksum
    );
    let summary = &report.summary["edit"];
    let total = summary.total.as_ref().unwrap();
    assert_eq!(total.samples, 2);
    assert_eq!(summary.inspection.as_ref().unwrap().samples, 2);
    let mut times: Vec<_> = edits.iter().map(|s| s.total).collect();
    times.sort_unstable();
    assert_eq!(total.minimum, times[0]);
    assert_eq!(total.maximum, times[1]);
    assert_eq!(total.p95, times[1]);
    assert_eq!(total.median, times[0] + (times[1] - times[0]) / 2);
    assert_eq!(summary.reference.as_ref().unwrap().exceeded, 2);
    assert!(summary.gpu.is_none());
    assert!(report.summary["seed"].reference.is_none());
    assert!(report.summary["steady"].preparation.is_none());
}

#[test]
fn settings() {
    for (key, value) in [
        ("scenario.count", serde_json::json!(1)),
        ("scenario.runs", serde_json::json!(0)),
        ("scenario.edits", serde_json::json!(0)),
        ("scenario.edits", serde_json::json!(31)),
        ("scenario.viewport", serde_json::json!([640])),
        ("scenario.reference", serde_json::json!(-1)),
    ] {
        let config = config::Store::default();
        config.set(key, value.clone());
        assert_eq!(config.get(key), Some(value));
        assert!(scenario::configured("list", options(), config).is_err());
    }
}
fn options() -> Options<'static> {
    Options {
        size: "small",
        mode: "--check",
        backend: "cpu",
        platform: ("test", "test"),
    }
}
