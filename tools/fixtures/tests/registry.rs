use fixtures::registry::{Registry, Role};
use std::path::PathBuf;

fn root() -> PathBuf {
    fixtures::root().parent().unwrap().to_owned()
}

fn registry() -> Registry {
    Registry::open(&root()).unwrap()
}

#[test]
fn coverage() {
    let registry = registry();
    assert_eq!(registry.applications.len(), 34);
    assert_eq!(
        registry
            .applications
            .values()
            .filter(|app| app.workload.is_some())
            .count(),
        9
    );
    assert!(registry.applications["list"].roles.len() > 1);
    for name in ["benchmarks", "fonts"] {
        let app = &registry.applications[name];
        assert!(app.roles.contains(&Role::Resource));
        assert!(app.control.is_none());
        assert!(app.content.is_none());
    }
}

#[test]
fn entries() {
    let registry = registry();
    assert_eq!(
        registry.applications["web"].content.as_deref(),
        Some("site/index.html")
    );
    assert!(registry.applications["site"].control.is_none());
    assert!(registry.applications["multilingual"].control.is_none());
    for name in ["clusters", "multilingual"] {
        let app = &registry.applications[name];
        let html = std::fs::read_to_string(root().join("apps").join(app.content.as_ref().unwrap()))
            .unwrap();
        assert!(!html.contains("<script"));
    }
    for (name, dependency) in [
        ("web", "site"),
        ("isolation", "list"),
        ("network", "pictures"),
        ("list", "fonts"),
    ] {
        assert!(
            registry.applications[name]
                .dependencies
                .contains(dependency)
        );
    }
}

#[test]
fn missing() {
    let mut registry = registry();
    registry.applications.get_mut("list").unwrap().content = Some("list/absent.html".into());
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("missing registry file")
    );
}

#[test]
fn dependency() {
    for (name, dependency) in [
        ("web", "site"),
        ("isolation", "list"),
        ("network", "pictures"),
        ("list", "fonts"),
    ] {
        let mut registry = registry();
        registry
            .applications
            .get_mut(name)
            .unwrap()
            .dependencies
            .remove(dependency);
        let error = registry.validate(&root()).unwrap_err();
        assert!(error.contains("dependency") || error.contains("dependencies"));
    }
    let mut registry = registry();
    registry
        .applications
        .get_mut("list")
        .unwrap()
        .dependencies
        .insert("pictures".into());
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("dependencies disagree")
    );
}

#[test]
fn workload() {
    let mut registry = registry();
    registry.applications.get_mut("list").unwrap().workload = Some("missing".into());
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("unregistered workload")
    );
}

#[test]
fn consumer() {
    let mut registry = registry();
    registry.applications.get_mut("list").unwrap().consumers[0].reference =
        "nonexistent consumer reference".into();
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("consumer reference")
    );
}

#[test]
fn invented() {
    let mut changed = registry();
    changed
        .applications
        .get_mut("multilingual")
        .unwrap()
        .control = Some("clusters/start.ts".into());
    assert!(changed.validate(&root()).is_err());
    let mut registry = registry();
    registry.applications.get_mut("web").unwrap().content = Some("site/style.css".into());
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("loaded content")
    );
}

#[test]
fn inventory() {
    let mut registry = registry();
    registry.applications.remove("site");
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("maintained directories")
    );
}

#[test]
fn association() {
    let mut registry = registry();
    registry
        .applications
        .get_mut("list")
        .unwrap()
        .roles
        .insert(Role::Resource);
    assert!(
        registry
            .validate(&root())
            .unwrap_err()
            .contains("no consumer")
    );
}
