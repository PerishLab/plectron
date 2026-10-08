use project::{Adapters, Kind, Module, Target, Workspace};

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
        ("apps/todo/main.js", vec!["boundary", "input"]),
        ("apps/chat/app.js", vec!["input", "scroll"]),
        ("apps/visual/main.js", vec!["scroll", "visual", "range"]),
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
