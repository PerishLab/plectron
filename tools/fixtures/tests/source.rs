use fixtures::source::{self, Role};
use project::Module;

fn root() -> std::path::PathBuf {
    fixtures::root().parent().unwrap().to_owned()
}

#[test]
fn completeness() {
    let audit = source::audit(&root()).unwrap();
    assert_eq!(audit.graphs, 88);
    assert_eq!(audit.exports, 6);
    assert_eq!(audit.scripts, 30);
    assert_eq!(
        audit
            .files
            .values()
            .filter(|role| **role != Role::Canonical)
            .count(),
        46
    );
    assert_eq!(audit.files["apps/list/scenario.js"], Role::Driver);
    assert_eq!(
        audit.files["packages/reactive/tests/engine.js"],
        Role::Language
    );
    assert_eq!(audit.files["tools/chromium/paired.js"], Role::Browser);
    assert_eq!(
        audit.files["tools/spec/resources/harness.js"],
        Role::Harness
    );
}

#[test]
fn copies() {
    for path in [
        "apps/todo/main.js",
        "apps/visual/start.js",
        "packages/engine/src/index.js",
        "packages/composition/dist/input.js",
        "tools/fixtures/generated.mjs",
        "packages/reactive/tests/new.js",
        "tools/spec/resources/upstream/nested/new.js",
    ] {
        assert!(source::role(path).is_err(), "{path}");
    }
}

#[test]
fn exports() {
    for path in [
        "packages/engine/src/index.js",
        "packages/composition/dist/input.js",
    ] {
        assert!(source::export(&Module::new(path).unwrap()).is_err());
    }
    assert!(source::export(&Module::new("packages/engine/src/index.ts").unwrap()).is_ok());
}

#[test]
fn paths() {
    for path in [
        ["exam", "ples/cards/main.js"].concat(),
        ["@plectron/", "design/input"].concat(),
    ] {
        assert!(source::stale(&path).is_err());
    }
    assert!(source::stale("apps/cards/main.ts").is_ok());
}

#[test]
fn consumers() {
    for (consumer, references) in [
        (
            "crates/lib/tests/reactive/source.rs",
            vec!["engine.js", "gc.js", "errors.js", "boundary.js"],
        ),
        (
            "tools/chromium/src/measure.rs",
            vec!["paired.js", "cards.js"],
        ),
        ("tools/spec/src/harness.rs", vec!["harness.js"]),
        (
            "crates/lib/tests/conformance/applications/composition/source.rs",
            vec!["apps/list/scenario.js"],
        ),
        (
            "tools/observe/src/live/application.rs",
            vec!["root.join(\"main.js\")", "let count = 0;"],
        ),
    ] {
        let source = std::fs::read_to_string(root().join(consumer)).unwrap();
        for reference in references {
            assert!(source.contains(reference), "{consumer}: {reference}");
        }
    }
}

#[test]
fn upstream() {
    let audit = source::audit(&root()).unwrap();
    let profile: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root().join("tools/spec/resources/profile.json")).unwrap(),
    )
    .unwrap();
    let declared: std::collections::BTreeSet<_> = profile["cases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|case| {
            format!(
                "tools/spec/resources/upstream/{}",
                case["file"].as_str().unwrap()
            )
        })
        .collect();
    let actual = audit
        .files
        .iter()
        .filter(|(_, role)| **role == Role::Upstream)
        .map(|(path, _)| path.clone())
        .collect();
    assert_eq!(declared, actual);
    for case in profile["cases"].as_array().unwrap() {
        assert!(!case["path"].as_str().unwrap().is_empty());
        assert!(!case["blob"].as_str().unwrap().is_empty());
    }
}

#[test]
fn markup() {
    let module = Module::new("apps/list/index.html").unwrap();
    let entries = source::scripts(
        &module,
        "<script src = './main.ts'></script><script src=\"scenario.js\"></script>",
    )
    .unwrap();
    assert_eq!(
        entries,
        [
            Module::new("apps/list/main.ts").unwrap(),
            Module::new("apps/list/scenario.js").unwrap()
        ]
    );
    assert!(source::scripts(&module, "<script src=\"main.js\"></script>").is_err());
    assert!(source::scripts(&module, "<script>run()</script>").is_err());
    assert!(
        source::scripts(
            &module,
            "<script src=\"./main.ts\"></script><script src=\"main.ts\"></script>"
        )
        .is_err()
    );
}
