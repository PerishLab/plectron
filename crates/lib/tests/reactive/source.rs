use project::{Adapters, Kind, Module, Target, Workspace};

#[test]
fn source() {
    let root = fixtures::root().join("../packages/reactive");
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("package.json")).unwrap()).unwrap();
    assert_eq!(manifest["exports"], "./src/index.ts");
    let modules = [
        "index",
        "scope",
        "tracking",
        "array",
        "boundary",
        "scheduler",
        "value",
    ];
    let cases = ["engine.js", "gc.js", "errors.js", "boundary.js"];
    let mut retained: Vec<_> = std::fs::read_dir(root.join("test"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    retained.sort();
    let mut expected = cases.map(String::from);
    expected.sort();
    assert_eq!(retained, expected);
    for case in cases {
        let mut workspace = Workspace::open(
            root.join("test").join(case),
            Adapters::new(typescript::Typescript, pt::Templates),
        )
        .unwrap();
        let linked = workspace.link(Target::Module).unwrap();
        assert_eq!(linked.graph.modules[&linked.graph.entry].kind, Kind::Js);
        for name in modules {
            assert!(!root.join("src").join(format!("{name}.js")).exists());
            let module = Module::new(format!("packages/reactive/src/{name}.ts")).unwrap();
            assert_eq!(linked.graph.modules[&module].kind, Kind::Ts);
            assert!(linked.origins.iter().any(|origin| origin.module == module));
        }
        assert!(
            linked
                .origins
                .iter()
                .any(|origin| origin.module == linked.graph.entry)
        );
    }
    for entry in std::fs::read_dir(root.join("tests")).unwrap() {
        assert_eq!(entry.unwrap().path().extension().unwrap(), "ts");
    }
}
