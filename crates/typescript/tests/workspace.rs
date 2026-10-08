use project::{Kind, Mapping, Module, Span, Target, Workspace};

#[test]
fn files() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join("packages/data")).unwrap();
    std::fs::create_dir(root.join("apps")).unwrap();
    std::fs::write(
        root.join("pnpm-workspace.yaml"),
        "packages:\n  - packages/*\n  - apps/*\n",
    )
    .unwrap();
    std::fs::write(
        root.join("packages/data/package.json"),
        r#"{"name":"data","exports":{".":"./value.ts"}}"#,
    )
    .unwrap();
    std::fs::write(
        root.join("packages/data/value.ts"),
        "export const value: number = 7;",
    )
    .unwrap();
    let source = "import type {Model} from './absent.ts'; import {type Shape, value} from 'data'; const answer: number = value; answer;";
    std::fs::write(root.join("apps/main.ts"), source).unwrap();
    let mut workspace = Workspace::open(
        root.join("apps/main.ts"),
        project::Adapters::new(typescript::Typescript, pt::Templates),
    )
    .unwrap();
    let initial = workspace.link(Target::Script).unwrap();
    let repeated = workspace.link(Target::Script).unwrap();
    assert_eq!(initial.graph, repeated.graph);
    assert_eq!(repeated.cache.hits, 2);
    let data = Module::new("packages/data/value.ts").unwrap();
    assert_eq!(initial.graph.modules[&data].kind, Kind::Ts);
    assert_eq!(initial.graph.modules.len(), 2);
    let start = initial.javascript.find("answer         = value").unwrap();
    let origin = initial
        .origin(Span {
            start,
            end: start + 6,
        })
        .unwrap();
    assert_eq!(origin.source.start, source.find("answer").unwrap());
    assert_eq!(origin.mapping, Mapping::Exact);
    std::fs::write(
        root.join("packages/data/value.ts"),
        "export const value: number = 9;",
    )
    .unwrap();
    let changed = workspace.link(Target::Script).unwrap();
    assert_ne!(initial.graph.identity, changed.graph.identity);
    assert_eq!(changed.cache.hits, 1);
    assert_eq!(changed.cache.misses, 1);
    assert!(changed.javascript.contains("value         = 9"));
}
