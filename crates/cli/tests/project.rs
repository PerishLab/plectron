#[path = "../src/project.rs"]
mod project;

#[test]
fn entry() {
    assert_eq!(project::path("entry = \"start.js\"").unwrap(), "start.js");
    assert!(project::path("entry = \"../start.js\"").is_err());
    assert!(project::path("entry = \"/tmp/start.js\"").is_err());
    assert!(project::path("entry = \"start.js\"\nextra = 1").is_err());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/todo");
    let source = project::load(&root).unwrap();
    assert!(source.contains("platform.serve(\"apps\")"));
    assert!(source.contains("Symbol[Symbol.plectron]"));
    assert!(!source.contains("import "));
}

#[test]
fn apps() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps");
    let mut starters = std::collections::BTreeSet::new();
    let mut manifests = std::collections::BTreeSet::new();
    for directory in std::fs::read_dir(root).unwrap() {
        let path = directory.unwrap().path();
        if path.join("start.js").is_file() {
            starters.insert(path.clone());
        }
        if path.join("plectron.toml").is_file() {
            manifests.insert(path);
        }
    }
    assert_eq!(starters, manifests);
    for path in manifests {
        assert!(!project::load(&path).unwrap().is_empty());
    }
}
