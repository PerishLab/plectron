use plectron_cli::project;

#[test]
fn entry() {
    assert_eq!(project::path("entry = \"start.js\"").unwrap(), "start.js");
    assert!(project::path("entry = \"../start.js\"").is_err());
    assert!(project::path("entry = \"/tmp/start.js\"").is_err());
    assert!(project::path("entry = \"start.js\"\nextra = 1").is_err());
    let root = fixtures::root().join("todo");
    let source = project::load(&root).unwrap();
    assert!(source.contains("platform.serve(\"apps\")"));
    assert!(source.contains("Symbol[Symbol.plectron]"));
    assert!(!source.contains("import "));
}

#[test]
fn apps() {
    let root = fixtures::root();
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

#[test]
fn typescript() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join("packages")).unwrap();
    std::fs::create_dir(root.join("app")).unwrap();
    std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - app\n").unwrap();
    std::fs::write(root.join("app/plectron.toml"), "entry = \"start.ts\"").unwrap();
    std::fs::write(root.join("app/start.ts"),
        "import type {Missing} from './absent.ts'; import {value} from './value.ts'; const answer: number = value;").unwrap();
    std::fs::write(root.join("app/value.ts"), "export const value: number = 7;").unwrap();
    let source = project::load(&root.join("app")).unwrap();
    assert!(!source.contains("import "));
    assert!(!source.contains(": number"));
    assert!(source.contains("value         = 7"));
}
