use project::{Kind, Module, Source};

fn source(text: &str) -> Source {
    Source {
        module: Module::new("main.ts").unwrap(),
        kind: Kind::Ts,
        text: text.into(),
    }
}

#[test]
fn erasable() {
    for text in [
        "const count: number = 1;",
        "type Pair<T> = [T, T]; interface Model<T> { value: T; }",
        "const identity = <T>(value: T): T => value; identity<number>(1);",
        "const value = (1 as number) satisfies number; value!;",
        "class Model<T> implements Shape<T> { public readonly value!: T; declare extra: number; constructor(value: T) { this.value = value; } }",
        "abstract class Model { abstract value: number; protected method(value?: number): number { return value!; } }",
        "function example(this: Model, value?: number, ...rest: number[]): number { return value!; }",
        "declare const ambient: number; declare function ambientCall(value: string): void;",
        "import type {Model} from './model.ts'; import {type Shape, value} from './model.ts'; export type {Shape}; export {value};",
        "type Select<T> = T extends infer U ? { [K in keyof U]: U[K] } : never;",
        "const text = 'enum namespace @decorator <tag>'; // enum Fake {}\nconst value: number = 1;",
    ] {
        typescript::check(&source(text)).unwrap_or_else(|error| panic!("{error}: {text}"));
    }
}

#[test]
fn excluded() {
    for (text, message) in [
        ("enum Model { One }", "enum"),
        ("const enum Model { One }", "enum"),
        ("declare enum Model { One }", "enum"),
        ("namespace Model { export const one = 1; }", "namespace"),
        ("declare module 'model' { interface Value {} }", "namespace"),
        (
            "export {}; declare global { interface Model {} }",
            "namespace",
        ),
        ("export as namespace Model;", "namespace"),
        (
            "class Model { constructor(public value: number) {} }",
            "parameter properties",
        ),
        (
            "class Model { constructor(readonly value: number) {} }",
            "parameter properties",
        ),
        ("import Model = require('model');", "assignments"),
        ("import type Model = require('model');", "assignments"),
        ("export = Model;", "assignments"),
        ("const value = <number>1;", "prefix"),
        ("@decorate class Model {}", "decorators"),
        ("class Model { @decorate value = 1; }", "decorators"),
        (
            "class Model { accessor value: number = 1; }",
            "auto-accessor",
        ),
    ] {
        let source = source(text);
        let failure = typescript::check(&source).expect_err(text);
        assert_eq!(failure.module, source.module);
        assert!(failure.message.contains(message), "{failure}: {text}");
        assert!(failure.span.start < failure.span.end, "{failure}: {text}");
        assert!(
            source
                .text
                .get(failure.span.start..failure.span.end)
                .is_some()
        );
    }
}

#[test]
fn nested() {
    let source = source("const text = '𐐀'; function outer() { enum Hidden { One } }");
    let failure = typescript::check(&source).unwrap_err();
    assert_eq!(failure.span.start, source.text.find("enum").unwrap());
    assert_eq!(
        &source.text[failure.span.start..failure.span.end],
        "enum Hidden { One }"
    );
    assert!(failure.message.contains("enum"));
}

#[test]
fn recovery() {
    for text in [
        "const value: = 1;",
        "const value = ;",
        "const value = <div />;",
    ] {
        let source = source(text);
        let failure = typescript::check(&source).unwrap_err();
        assert_eq!(failure.module, source.module);
        assert!(
            source
                .text
                .get(failure.span.start..failure.span.end)
                .is_some(),
            "{failure}"
        );
    }
}

#[test]
fn semantics() {
    for text in ["const value = 1; const value = 2;", "break;", "return 1;"] {
        assert!(typescript::check(&source(text)).is_err(), "{text}");
    }
}

#[test]
fn boundary() {
    let mut source = source("const value = 1;");
    source.kind = Kind::Js;
    assert!(
        typescript::check(&source)
            .unwrap_err()
            .message
            .contains("expected TypeScript")
    );
    source.kind = Kind::Pt;
    assert!(typescript::check(&source).is_err());
}

#[test]
fn positions() {
    let source = source("const text = '𐐀';\nconst value: = 1;");
    let failure = typescript::check(&source).unwrap_err();
    assert!(failure.span.start >= source.text.find("value").unwrap());
    assert!(
        source
            .text
            .get(failure.span.start..failure.span.end)
            .is_some()
    );
}

#[test]
fn arrows() {
    for text in [
        "const result = (value: number)\n: number => value;",
        "const result = (value: number):\nnumber => value;",
        "const result = (value: number) /*\n*/: number => value;",
        "const value = 1 as number\nconst other = 2;",
    ] {
        typescript::check(&source(text)).unwrap_or_else(|error| panic!("{error}: {text}"));
    }
}

#[test]
fn upstream() {
    for text in [
        "enum Model { One }",
        "namespace Model {}",
        "class Model { constructor(public value: number) {} }",
        "import Model = require('model');",
        "export = Model;",
        "const value = <number>1;",
        "@decorate class Model {}",
    ] {
        let arena = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(&arena, text, oxc_span::SourceType::ts()).parse();
        assert!(!parsed.fatal_error, "{text}");
        assert!(parsed.diagnostics.is_empty(), "{text}");
        assert!(typescript::check(&source(text)).is_err(), "{text}");
    }
    let arena = oxc_allocator::Allocator::default();
    let text = "const value = <div />;";
    let parsed = oxc_parser::Parser::new(&arena, text, oxc_span::SourceType::tsx()).parse();
    assert!(parsed.diagnostics.is_empty());
    assert!(typescript::check(&source(text)).is_err());
}
