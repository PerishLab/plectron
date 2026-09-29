use project::{Javascript, Linker, Memory, Module, Resolver, Target, Workspace};

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

#[test]
fn providers() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(directory.path().join("pnpm-workspace.yaml"), "packages: []").unwrap();
    std::fs::create_dir(directory.path().join("packages")).unwrap();
    std::fs::create_dir(directory.path().join("apps")).unwrap();
    let main = "import { value } from \"./data.js\"; export const result = value;";
    let data = "export const value = 7;";
    std::fs::write(directory.path().join("apps/main.js"), main).unwrap();
    std::fs::write(directory.path().join("apps/data.js"), data).unwrap();

    let mut files = Workspace::open(directory.path().join("apps/main.js"), Javascript).unwrap();
    let filesystem = files.link(Target::Script).unwrap();
    let memory = Memory::new([
        (module("apps/main.js"), main.into()),
        (module("apps/data.js"), data.into()),
    ]);
    let mut virtualized = Linker::new(memory, Resolver::default(), Javascript);
    let virtualized = virtualized
        .link(module("apps/main.js"), Target::Script)
        .unwrap();

    assert_eq!(filesystem.graph, virtualized.graph);
    assert_eq!(filesystem.javascript, virtualized.javascript);
}
