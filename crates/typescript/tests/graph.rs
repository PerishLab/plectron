use jsengine::{Addons, Engine, Failure, Invocation, Value};
use project::{Kind, Linker, Memory, Module, Package, Resolver, Target};

struct Bare;

impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

fn adapters() -> project::Adapters<typescript::Typescript, pt::Templates> {
    project::Adapters::new(typescript::Typescript, pt::Templates)
}

fn execute(output: &project::Output) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(&output.javascript, &mut Bare)
        .unwrap_or_else(|error| panic!("{}: {}", engine.diagnostic(&error), output.javascript))
}

#[test]
fn composition() {
    let memory = Memory::new([
        (module("main.ts"), "import {value} from './value.js'; import Template from './view.pt'; const document = {createElement: (tag) => ({tag: tag, appendChild: (child) => {}}), createTextNode: (text) => ({text: text})}; const view = Template(document); value + view.host.tag.length - 2;".into()),
        (module("value.js"), "import {base} from './base.ts'; export const value = base;".into()),
        (module("base.ts"), "export const base: number = 6;".into()),
        (module("view.pt"), "<template tag=\"div\">hello</template>".into()),
    ]);
    let output = Linker::new(memory, Resolver::default(), adapters())
        .link(module("main.ts"), Target::Script)
        .unwrap();
    assert_eq!(execute(&output), Value::Number(7.0));
    assert_eq!(output.graph.modules.len(), 4);
    for (path, kind) in [
        ("main.ts", Kind::Ts),
        ("base.ts", Kind::Ts),
        ("value.js", Kind::Js),
        ("view.pt", Kind::Pt),
    ] {
        assert_eq!(output.graph.modules[&module(path)].kind, kind);
    }
}

#[test]
fn declarations() {
    for declaration in [
        "import type {Model} from './missing.ts';",
        "import type Model from './missing.ts';",
        "import type * as Model from './missing.ts';",
        "export type {Model} from './missing.ts';",
        "export type * from './missing.ts';",
        "export type * as Model from './missing.ts';",
    ] {
        let memory = Memory::new([(module("main.ts"), format!("{declaration} 7;"))]);
        let output = Linker::new(memory, Resolver::default(), adapters())
            .link(module("main.ts"), Target::Script)
            .unwrap();
        assert_eq!(execute(&output), Value::Number(7.0));
        assert_eq!(output.graph.modules.len(), 1);
        assert!(
            output.graph.modules[&module("main.ts")]
                .dependencies
                .is_empty()
        );
    }
}

#[test]
fn effects() {
    for statement in [
        "import {type Model} from './effects.ts';",
        "import {type Model, value} from './effects.ts';",
        "export {type Model} from './effects.ts';",
        "export {type Model, value} from './effects.ts';",
        "import './effects.ts';",
        "import {value} from './effects.ts';",
    ] {
        let memory = Memory::new([
            (module("main.ts"), format!("import {{state}} from './state.js'; {statement} state.count;")),
            (module("state.js"), "export const state = {count: 0};".into()),
            (module("effects.ts"), "import {state} from './state.js'; state.count++; export type Model = number; export const value: number = 7;".into()),
        ]);
        let output = Linker::new(memory, Resolver::default(), adapters())
            .link(module("main.ts"), Target::Script)
            .unwrap();
        assert_eq!(execute(&output), Value::Number(1.0), "{statement}");
        assert_eq!(output.graph.modules.len(), 3);
        assert_eq!(
            output.graph.modules[&module("main.ts")].dependencies.len(),
            2
        );
    }
}

#[test]
fn exports() {
    let package = Package::parse(
        module("packages/data"),
        r#"{"name":"data","exports":{".":"./value.ts"}}"#,
    )
    .unwrap();
    let memory = Memory::new([
        (
            module("main.js"),
            "import {value} from 'data'; value;".into(),
        ),
        (
            module("packages/data/value.ts"),
            "export const value: number = 7;".into(),
        ),
    ]);
    let output = Linker::new(memory, Resolver::new([package]).unwrap(), adapters())
        .link(module("main.js"), Target::Script)
        .unwrap();
    assert_eq!(execute(&output), Value::Number(7.0));
    assert_eq!(output.graph.order[0], module("packages/data/value.ts"));
    assert_eq!(
        output.graph.modules[&module("packages/data/value.ts")].kind,
        Kind::Ts
    );
}

#[test]
fn caching() {
    let memory = Memory::new([
        (
            module("main.js"),
            "import {value} from './value.ts'; value;".into(),
        ),
        (module("value.ts"), "export const value: number = 7;".into()),
    ]);
    let mut linker = Linker::new(memory, Resolver::default(), adapters());
    let initial = linker.link(module("main.js"), Target::Script).unwrap();
    let repeated = linker.link(module("main.js"), Target::Script).unwrap();
    assert_eq!(initial.graph, repeated.graph);
    assert_eq!(repeated.cache.hits, 2);
    linker
        .provider()
        .insert(module("value.ts"), "export const value: string = 7;");
    let changed = linker.link(module("main.js"), Target::Script).unwrap();
    assert_eq!(initial.javascript, changed.javascript);
    assert_ne!(initial.graph.identity, changed.graph.identity);
    assert_ne!(
        initial.graph.modules[&module("value.ts")].content,
        changed.graph.modules[&module("value.ts")].content
    );
    assert_eq!(changed.cache.hits, 1);
    assert_eq!(changed.cache.misses, 1);
    assert_eq!(execute(&changed), Value::Number(7.0));
    linker
        .provider()
        .insert(module("value.ts"), "export const value: number = 9;");
    let changed = linker.link(module("main.js"), Target::Script).unwrap();
    assert_eq!(execute(&changed), Value::Number(9.0));
}

#[test]
fn reexports() {
    let memory = Memory::new([
        (module("main.js"), "import {answer, local} from './bridge.ts'; answer + local;".into()),
        (module("bridge.ts"), "export {type Model, value as answer} from './value.ts'; const module_reexport_0_0 = 1; const value: number = 1; export const local: number = value;".into()),
        (module("value.ts"), "export type Model = number; export const value: number = 6;".into()),
    ]);
    let output = Linker::new(memory, Resolver::default(), adapters())
        .link(module("main.js"), Target::Script)
        .unwrap();
    assert_eq!(execute(&output), Value::Number(7.0));
    assert!(output.javascript.contains("const module_reexport_0_0_ ="));
}

#[test]
fn templates() {
    let source = "<template tag=\"div\"><span ref=\"first\"></span></template>";
    let memory = Memory::new([
        (
            module("main.ts"),
            "import Template from './view.pt'; const view = Template; 7;".into(),
        ),
        (module("view.pt"), source.into()),
    ]);
    let mut linker = Linker::new(memory, Resolver::default(), adapters());
    let initial = linker.link(module("main.ts"), Target::Script).unwrap();
    let repeated = linker.link(module("main.ts"), Target::Script).unwrap();
    assert_eq!(initial.graph, repeated.graph);
    assert_eq!(repeated.cache.hits, 2);
    let origin = initial
        .origins
        .iter()
        .find(|origin| origin.module == module("view.pt"))
        .unwrap();
    assert_eq!(
        origin.source,
        project::Span {
            start: 0,
            end: source.len()
        }
    );
    assert_eq!(origin.mapping, project::Mapping::Range);
    linker.provider().insert(
        module("view.pt"),
        "<template tag=\"div\"><span ref=\"second\"></span></template>",
    );
    let changed = linker.link(module("main.ts"), Target::Script).unwrap();
    assert_ne!(initial.graph.identity, changed.graph.identity);
    assert_eq!(changed.cache.hits, 1);
    assert_eq!(changed.cache.misses, 1);
    assert!(changed.javascript.contains("refs.second"));
    assert!(!changed.javascript.contains("refs.first"));
}

#[test]
fn cycles() {
    for entry in ["main.ts", "main.js"] {
        let memory = Memory::new([
            (module(entry), "import Template from './view.pt'; import {value} from './value.ts'; export const state = value;".into()),
            (module("value.ts"), format!("import {{state}} from './{entry}'; export const value: number = state;")),
            (module("view.pt"), "<template tag=\"div\"></template>".into()),
        ]);
        let failure = Linker::new(memory, Resolver::default(), adapters())
            .link(module(entry), Target::Script)
            .unwrap_err();
        assert!(failure.message.starts_with("module cycle:"));
        assert!(failure.message.contains(entry) && failure.message.contains("value.ts"));
    }
}
