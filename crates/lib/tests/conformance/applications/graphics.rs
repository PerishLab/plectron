use project::{Adapters, Kind, Module, Target, Workspace};

#[test]
fn canonical() {
    let root = fixtures::root().parent().unwrap().to_owned();
    for name in [
        "article",
        "atoms",
        "cards",
        "direction",
        "flex",
        "gallery",
        "inline",
        "pictures",
        "continuation",
        "reshaping",
        "typography",
        "wrapping",
    ] {
        let directory = root.join("apps").join(name);
        let html = std::fs::read_to_string(directory.join("index.html")).unwrap();
        let manifest = std::fs::read_to_string(directory.join("plectron.toml")).unwrap();
        assert!(html.contains("main.ts"));
        assert!(manifest.contains("entry = \"start.ts\""));
        for entry in ["main", "start"] {
            assert!(!directory.join(format!("{entry}.js")).exists());
            let path = directory.join(format!("{entry}.ts"));
            let mut workspace =
                Workspace::open(&path, Adapters::new(typescript::Typescript, pt::Templates))
                    .unwrap();
            let output = workspace.link(Target::Script).unwrap();
            let module = Module::new(format!("apps/{name}/{entry}.ts")).unwrap();
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
    }
}
