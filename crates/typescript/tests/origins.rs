use project::{Adapter, Kind, Linker, Mapping, Memory, Module, Resolver, Source, Span, Target};

fn module(path: &str) -> Module {
    Module::new(path).unwrap()
}

fn occurrence(text: &str, token: &str) -> Span {
    let start = text.find(token).unwrap();
    Span {
        start,
        end: start + token.len(),
    }
}

fn translate(text: &str) -> project::Artifact {
    typescript::Typescript
        .translate(Source {
            module: module("main.ts"),
            kind: Kind::Ts,
            text: text.into(),
        })
        .unwrap()
}

#[test]
fn boundary() {
    for kind in [Kind::Js, Kind::Pt] {
        let error = typescript::Typescript
            .translate(Source {
                module: module("main.ts"),
                kind,
                text: "7;".into(),
            })
            .unwrap_err();
        assert_eq!(error.module, module("main.ts"));
        assert_eq!(error.message, "expected TypeScript source");
    }
}

#[test]
fn unicode() {
    let text = "type 名字 = '𐐀'; const message: 名字 = '𐐀'; message;";
    let artifact = translate(text);
    assert_eq!(artifact.kind, Kind::Js);
    assert_eq!(artifact.text.len(), text.len());
    for origin in &artifact.origins {
        assert!(artifact.text.is_char_boundary(origin.generated.start));
        assert!(artifact.text.is_char_boundary(origin.generated.end));
        assert!(text.is_char_boundary(origin.source.start));
        assert!(text.is_char_boundary(origin.source.end));
    }
    let span = occurrence(text, "'𐐀'; message");
    assert_eq!(artifact.origin(span).source, span);
    assert_eq!(artifact.origin(span).mapping, Mapping::Exact);
    let erased = occurrence(text, "名字");
    assert_eq!(artifact.origin(erased).mapping, Mapping::Range);
    let spanning = Span {
        start: 0,
        end: text.len(),
    };
    assert_eq!(artifact.origin(spanning).source, spanning);
    assert_eq!(artifact.origin(spanning).mapping, Mapping::Range);
}

#[test]
fn repairs() {
    for text in [
        "const pick = (value: number)\n: 类型 => value; pick(7);",
        "let result = 0; 1 as 类型\n(result = 7); result;",
    ] {
        let lowered = typescript::erase(&Source {
            module: module("main.ts"),
            kind: Kind::Ts,
            text: text.into(),
        })
        .unwrap();
        let artifact = translate(text);
        assert!(!lowered.repairs.is_empty());
        let memory = Memory::new([(module("main.ts"), text.into())]);
        let output = Linker::new(memory, Resolver::default(), typescript::Typescript)
            .link(module("main.ts"), Target::Script)
            .unwrap();
        for repair in lowered.repairs {
            let origin = artifact.origin(repair.generated);
            assert_eq!(origin.source, repair.source);
            assert_eq!(origin.mapping, Mapping::Range);
            assert!(
                output
                    .origins
                    .iter()
                    .any(|origin| origin.mapping == Mapping::Range
                        && origin.source == repair.source
                        && origin.module == module("main.ts"))
            );
        }
    }
}

#[test]
fn linking() {
    let text = "import {type Model, value} from './value.ts'; const message: string = '雪'; value + message;";
    let memory = Memory::new([
        (module("main.ts"), text.into()),
        (module("value.ts"), "export const value: number = 7;".into()),
    ]);
    let output = Linker::new(memory, Resolver::default(), typescript::Typescript)
        .link(module("main.ts"), Target::Script)
        .unwrap();
    let origin = output
        .origin(occurrence(&output.javascript, "'雪'"))
        .unwrap();
    assert_eq!(origin.module, module("main.ts"));
    assert_eq!(origin.source, occurrence(text, "'雪'"));
    assert_eq!(origin.mapping, Mapping::Exact);
    assert_eq!(
        output.graph.modules[&module("main.ts")].dependencies[0].span,
        occurrence(text, "./value.ts")
    );
    assert!(
        output
            .origin(occurrence(&output.javascript, "return {"))
            .is_none()
    );
}

#[test]
fn diagnostics() {
    let text = "const message: string = '雪'; import {type Model, value} from './private';";
    let memory = Memory::new([(module("main.ts"), text.into())]);
    let error = Linker::new(memory, Resolver::default(), typescript::Typescript)
        .link(module("main.ts"), Target::Script)
        .unwrap_err();
    assert_eq!(error.module, module("main.ts"));
    assert_eq!(error.span, occurrence(text, "./private"));
    let invalid = "const message = '雪'; enum Bad { Value }";
    let memory = Memory::new([
        (module("main.js"), "import './bad.ts';".into()),
        (module("bad.ts"), invalid.into()),
    ]);
    let error = Linker::new(
        memory,
        Resolver::default(),
        project::Adapters::new(typescript::Typescript, project::Unavailable),
    )
    .link(module("main.js"), Target::Script)
    .unwrap_err();
    assert_eq!(error.module, module("bad.ts"));
    assert!(error.span.start >= invalid.find("enum").unwrap());
    assert!(invalid.is_char_boundary(error.span.start));
    assert!(invalid.is_char_boundary(error.span.end));
}
