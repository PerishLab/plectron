use plectron_cli::project;

#[test]
fn entry() {
    assert_eq!(project::path("entry = \"start.js\"").unwrap(), "start.js");
    assert!(project::path("entry = \"../start.js\"").is_err());
    assert!(project::path("entry = \"/tmp/start.js\"").is_err());
    assert!(project::path("entry = \"start.js\"\nextra = 1").is_err());
    let root = fixtures::root().join("todo");
    let source = project::load(&root).unwrap();
    let source = source.source().text();
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
        if path.join("src/start.js").is_file() || path.join("src/start.ts").is_file() {
            starters.insert(path.clone());
        }
        if path.join("plectron.toml").is_file() {
            manifests.insert(path);
        }
    }
    assert_eq!(starters, manifests);
    for path in manifests {
        assert!(!project::load(&path).unwrap().source().text().is_empty());
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
    let source = source.source().text();
    assert!(!source.contains("import "));
    assert!(!source.contains(": number"));
    assert!(source.contains("value         = 7"));
}

#[test]
fn execution() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir(root.join("packages")).unwrap();
    std::fs::create_dir(root.join("app")).unwrap();
    std::fs::write(root.join("pnpm-workspace.yaml"), "packages:\n  - app\n").unwrap();
    std::fs::write(root.join("app/plectron.toml"), "entry = \"start.ts\"").unwrap();
    std::fs::write(
        root.join("app/start.ts"),
        "import {fail} from './value.ts'; const answer: number = 7; const saved = fail;",
    )
    .unwrap();
    let definition = "export const fail = function (): never { throw 'retained'; };";
    std::fs::write(root.join("app/value.ts"), definition).unwrap();
    let script = project::file(&root.join("app/start.ts")).unwrap();
    let (proxy, _) = plectron::control::proxy::Proxy::channel();
    let mut runtime =
        plectron::runtime::Runtime::new(plectron::addons::control::Control::new(proxy)).unwrap();
    runtime.script(&script).unwrap();
    drop(script);
    assert_eq!(runtime.execute("answer;").unwrap().text(), "7");
    runtime.collect().unwrap();
    let failure = runtime.execute("saved();").unwrap_err();
    let location = runtime.location(&failure).unwrap();
    assert_eq!(location.resource, "app/value.ts");
    assert_eq!(&definition[location.span], "throw 'retained';");
}
