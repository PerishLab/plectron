use project::{
    Adapter, Artifact, Diagnostic, Kind, Linker, Memory, Module, Origin, Resolver, Source, Span,
    Target,
};

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

fn sources(reverse: bool) -> Memory {
    let mut sources = vec![
        (
            module("main.js"),
            "import { value } from \"./data.js\"; import \"./data.js\"; export const result = value;"
                .into(),
        ),
        (module("data.js"), "export const value = 7;".into()),
    ];
    if reverse {
        sources.reverse();
    }
    Memory::new(sources)
}

#[test]
fn caching() {
    let entry = module("main.js");
    let mut first = Linker::new(sources(false), Resolver::default(), project::Javascript);
    let initial = first.link(entry.clone(), Target::Module).unwrap();
    let repeated = first.link(entry.clone(), Target::Module).unwrap();
    let mut reordered = Linker::new(sources(true), Resolver::default(), project::Javascript);
    let equivalent = reordered.link(entry, Target::Module).unwrap();

    assert_eq!(initial.graph.identity, repeated.graph.identity);
    assert_eq!(initial.graph.identity, equivalent.graph.identity);
    assert_eq!(initial.javascript, equivalent.javascript);
    assert_eq!(initial.graph.modules.len(), 2);
    assert_eq!(initial.graph.order.len(), 2);
    assert_eq!(initial.cache.misses, 2);
    assert_eq!(repeated.cache.hits, 2);
    assert_eq!(repeated.cache.entries, 2);
    assert_eq!(
        initial.graph.modules[&module("main.js")].dependencies.len(),
        2
    );
}

#[test]
fn invalidation() {
    let entry = module("main.js");
    let mut linker = Linker::new(sources(false), Resolver::default(), project::Javascript);
    let initial = linker.link(entry.clone(), Target::Module).unwrap();
    linker
        .provider()
        .insert(module("data.js"), "export const value = 9;");
    let changed = linker.link(entry, Target::Module).unwrap();

    assert_ne!(initial.graph.identity, changed.graph.identity);
    assert_eq!(changed.cache.hits, 1);
    assert_eq!(changed.cache.misses, 1);
    assert!(changed.javascript.contains("value = 9"));
}

#[test]
fn cycles() {
    let memory = Memory::new([
        (
            module("a.js"),
            "import \"./b.js\"; export const a = 1;".into(),
        ),
        (
            module("b.js"),
            "import \"./a.js\"; export const b = 2;".into(),
        ),
    ]);
    let mut linker = Linker::new(memory, Resolver::default(), project::Javascript);
    let failure = linker.link(module("a.js"), Target::Module).unwrap_err();
    assert_eq!(failure.module, module("b.js"));
    assert_eq!(failure.message, "module cycle: a.js -> b.js -> a.js");
    assert!(failure.span.end > failure.span.start);
}

struct Generated;

impl Adapter for Generated {
    fn translate(&self, source: Source) -> Result<Artifact, Diagnostic> {
        let text = "import {".to_string();
        Ok(Artifact {
            module: source.module.clone(),
            kind: Kind::Js,
            origins: vec![Origin {
                generated: Span {
                    start: 0,
                    end: text.len(),
                },
                module: source.module,
                source: Span { start: 4, end: 8 },
            }],
            text,
        })
    }
}

#[test]
fn origins() {
    let memory = Memory::new([(module("view.pt"), "template".into())]);
    let mut linker = Linker::new(memory, Resolver::default(), Generated);
    let failure = linker.link(module("view.pt"), Target::Module).unwrap_err();
    assert_eq!(failure.module, module("view.pt"));
    assert_eq!(failure.span, Span { start: 4, end: 8 });
}
