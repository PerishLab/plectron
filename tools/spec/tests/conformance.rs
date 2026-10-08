use spec::{Profile, Selection, suite};

#[test]
fn upstream() {
    let profile = Profile::load().unwrap();
    let report = suite::run(profile).unwrap();
    assert_eq!(report.totals[&suite::Status::Failed], 0, "{report:?}");
    assert_eq!(
        report.totals[&suite::Status::Passed],
        profile
            .cases
            .iter()
            .filter(|case| case.selection == Selection::Run)
            .count()
    );
    assert!(report.totals[&suite::Status::Passed] > 0);
    assert_eq!(report.cases.len(), profile.cases.len());
    assert!(report.totals[&suite::Status::Excluded] > 0);
    assert!(report.totals[&suite::Status::Unsupported] > 0);
    assert_eq!(
        report.totals[&suite::Status::Blocked],
        profile
            .cases
            .iter()
            .filter(|case| case.selection == Selection::Blocked)
            .count()
    );
}

#[test]
fn integrity() {
    let profile = Profile::load().unwrap();
    assert!(suite::verify(&profile.cases[0].file, "changed").is_err());
    assert!(suite::verify("../profile.json", "changed").is_err());
}

#[test]
fn source() {
    let profile = Profile::load().unwrap();
    let modules = profile
        .capabilities
        .iter()
        .find(|capability| capability.id == "modules")
        .unwrap();
    assert_eq!(serde_json::to_value(&modules.state).unwrap(), "pending");
    assert!(modules.implementation.is_empty() && modules.evidence.is_empty());
    let exceptions = profile
        .capabilities
        .iter()
        .find(|capability| capability.id == "exceptions")
        .unwrap();
    assert_eq!(serde_json::to_value(&exceptions.state).unwrap(), "subset");
    assert!(
        exceptions
            .evidence
            .iter()
            .any(|path| path == "tools/fixtures/tests/source.rs")
    );
    assert!(
        exceptions
            .gaps
            .iter()
            .any(|gap| gap.contains("原生入口端到端验收尚未完成"))
    );
}

#[test]
fn budget() {
    let source = serde_json::to_string(Profile::load().unwrap()).unwrap();
    let mut profile: Profile = serde_json::from_str(&source).unwrap();
    profile.reference = 1;
    let root = std::env::temp_dir().join(format!("plectron-budget-{}", std::process::id()));
    std::fs::create_dir_all(root.join("crates/toy/src")).unwrap();
    std::fs::create_dir_all(root.join("tools/spec/resources/upstream")).unwrap();
    std::fs::create_dir_all(root.join("target")).unwrap();
    std::fs::write(root.join("crates/toy/src/lib.rs"), "one\ntwo\n").unwrap();
    std::fs::write(root.join("target/ignored.rs"), "build\n").unwrap();
    let upstream = root.join("tools/spec/resources/upstream");
    std::fs::write(upstream.join(&profile.cases[0].file), "upstream\n").unwrap();
    std::fs::write(upstream.join("unlisted.rs"), "owned\n").unwrap();
    let budget = spec::budget::measure(&root, &profile).unwrap();
    assert_eq!(budget.reference, 1);
    assert_eq!(budget.maintained, 3);
    assert_eq!(budget.upstream, 1);
    std::fs::remove_dir_all(root).unwrap();
}
