use project::{Kind, Linker, Memory, Module, Resolver, Span, Target};

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

#[test]
fn adapter() {
    let memory = Memory::new([
        (
            module("main.js"),
            "import Template from \"./view.pt\"; export const view = Template;".into(),
        ),
        (
            module("view.pt"),
            "<template tag=\"div\">hello</template>".into(),
        ),
    ]);
    let mut linker = Linker::new(memory, Resolver::default(), pt::Templates);
    let output = linker.link(module("main.js"), Target::Module).unwrap();

    assert_eq!(output.graph.modules[&module("main.js")].kind, Kind::Js);
    assert_eq!(output.graph.modules[&module("view.pt")].kind, Kind::Pt);
    assert_eq!(output.graph.modules.len(), 2);
    let origin = output
        .origins
        .iter()
        .find(|origin| origin.module == module("view.pt"))
        .unwrap();
    assert_eq!(origin.source, Span { start: 0, end: 36 });
    let mapped = output.origin(origin.generated).unwrap();
    assert_eq!(mapped.module, module("view.pt"));
    assert_eq!(mapped.source, Span { start: 0, end: 36 });
}
