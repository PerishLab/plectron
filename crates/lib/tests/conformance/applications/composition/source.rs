use project::{Adapters, Kind, Module, Target, Workspace};

#[test]
fn workloads() {
    let root = fixtures::root().parent().unwrap().to_owned();
    for name in [
        "clusters",
        "convergence",
        "isolation",
        "layers",
        "scroll",
        "spaces",
        "visual",
        "typescript",
    ] {
        let directory = root.join("apps").join(name);
        let manifest = std::fs::read_to_string(directory.join("plectron.toml")).unwrap();
        assert!(manifest.contains("entry = \"src/start.ts\""));
        for entry in ["main", "start", "model"] {
            assert!(!directory.join("src").join(format!("{entry}.js")).exists());
            let path = directory.join("src").join(format!("{entry}.ts"));
            if !path.exists() {
                assert_ne!(entry, "start");
                continue;
            }
            let mut workspace =
                Workspace::open(&path, Adapters::new(typescript::Typescript, pt::Templates))
                    .unwrap();
            let output = workspace.link(Target::Script).unwrap();
            let module = Module::new(format!("apps/{name}/src/{entry}.ts")).unwrap();
            assert_eq!(output.graph.modules[&module].kind, Kind::Ts);
            assert!(
                output
                    .graph
                    .modules
                    .values()
                    .all(|module| module.kind != Kind::Js)
            );
            assert!(output.origins.iter().any(|origin| origin.module == module));
        }
        let html = std::fs::read_to_string(directory.join("src/index.html")).unwrap();
        if directory.join("src/main.ts").exists() {
            assert!(html.contains("main.ts"));
            assert!(!html.contains("main.js"));
        } else if name == "isolation" {
            assert!(html.contains("../../list/src/main.ts"));
            assert!(html.contains("../../list/src/scenario.js"));
            assert!(!directory.join("src/main.ts").exists());
        } else {
            assert_eq!(name, "clusters");
            assert!(!html.contains("<script"));
        }
    }
}

#[test]
fn staticness() {
    let root = fixtures::root().parent().unwrap().to_owned();
    for name in ["clusters", "multilingual"] {
        let directory = root.join("apps").join(name);
        let html = std::fs::read_to_string(directory.join("src/index.html")).unwrap();
        assert!(!html.contains("<script"));
        assert!(!directory.join("src/main.ts").exists());
        assert!(!directory.join("src/main.js").exists());
    }
}

const APPS: [&str; 11] = [
    "counter",
    "closure",
    "web",
    "todo",
    "list",
    "desk",
    "chat",
    "lifecycle",
    "network",
    "request",
    "site",
];

#[test]
fn canonical() {
    let root = fixtures::root().parent().unwrap().to_owned();
    for app in APPS {
        let directory = root.join("apps").join(app);
        let manifest = directory.join("plectron.toml");
        if manifest.is_file() {
            assert!(directory.join("src/start.ts").is_file());
            assert!(
                std::fs::read_to_string(manifest)
                    .unwrap()
                    .contains("entry = \"src/start.ts\"")
            );
        }
        let html = directory.join("src/index.html");
        if html.is_file() {
            assert!(directory.join("src/main.ts").is_file());
            assert!(std::fs::read_to_string(html).unwrap().contains("main.ts"));
        }
        for entry in std::fs::read_dir(directory.join("src")).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("js") {
                assert_eq!(path, root.join("apps/list/src/scenario.js"));
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("ts") {
                continue;
            }
            let mut files =
                Workspace::open(&path, Adapters::new(typescript::Typescript, pt::Templates))
                    .unwrap();
            let output = files.link(Target::Script).unwrap();
            assert!(!output.javascript.contains("import "));
            assert!(
                output
                    .graph
                    .modules
                    .values()
                    .all(|module| module.kind != Kind::Js)
            );
            let module = Module::new(format!(
                "apps/{app}/src/{}",
                path.file_name().unwrap().to_str().unwrap()
            ))
            .unwrap();
            assert_eq!(output.graph.modules[&module].kind, Kind::Ts);
        }
    }
}

#[test]
fn browser() {
    let root = fixtures::root().parent().unwrap().to_owned();
    for page in ["list", "isolation"] {
        let html =
            std::fs::read_to_string(root.join("apps").join(page).join("src/index.html")).unwrap();
        assert!(html.contains("main.ts"));
        assert!(html.contains("scenario.js"));
    }
    let mut files = Workspace::open(
        root.join("apps/list/src/scenario.js"),
        Adapters::new(typescript::Typescript, pt::Templates),
    )
    .unwrap();
    let output = files.link(Target::Script).unwrap();
    let module = Module::new("apps/list/src/scenario.js").unwrap();
    assert_eq!(output.graph.modules[&module].kind, Kind::Js);
    assert!(output.origins.iter().any(|origin| origin.module == module));
    let harness =
        std::fs::read_to_string(root.join("tools/chromium/src/reference/paired.js")).unwrap();
    assert!(harness.contains("scenario.seed("));
    assert!(harness.contains("scenario.run("));
}

#[test]
fn graph() {
    let root = fixtures::root().parent().unwrap().to_owned();
    let package = root.join("packages/composition");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(package.join("package.json")).unwrap()).unwrap();
    for name in ["boundary", "input", "scroll", "visual"] {
        assert_eq!(
            manifest["exports"][format!("./{name}")],
            format!("./src/{name}.ts")
        );
        assert!(!package.join("src").join(format!("{name}.js")).exists());
    }
    assert!(!package.join("test").exists());
    for (entry, names) in [
        ("apps/todo/src/main.ts", vec!["boundary", "input"]),
        ("apps/chat/src/app.ts", vec!["input", "scroll"]),
        ("apps/visual/src/main.ts", vec!["scroll", "visual", "range"]),
    ] {
        let mut workspace = Workspace::open(
            root.join(entry),
            Adapters::new(typescript::Typescript, pt::Templates),
        )
        .unwrap();
        let linked = workspace.link(Target::Module).unwrap();
        for name in names {
            let module = Module::new(format!("packages/composition/src/{name}.ts")).unwrap();
            assert_eq!(linked.graph.modules[&module].kind, Kind::Ts);
            assert!(linked.origins.iter().any(|origin| origin.module == module));
        }
        if !entry.starts_with("apps/visual") {
            let module = Module::new("packages/composition/src/input.pt").unwrap();
            assert_eq!(linked.graph.modules[&module].kind, Kind::Pt);
            assert!(linked.origins.iter().any(|origin| origin.module == module));
        }
    }
}
