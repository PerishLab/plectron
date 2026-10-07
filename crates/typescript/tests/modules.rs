use oxc_allocator::Allocator;
use oxc_parser::{Parser, config::TokensParserConfig};
use oxc_span::SourceType;
use project::{Kind, Module, Source};

fn lower(text: &str) -> typescript::Lowered {
    let source = Source {
        module: Module::new("module.ts").unwrap(),
        kind: Kind::Ts,
        text: text.into(),
    };
    let lowered = typescript::erase(&source).unwrap_or_else(|failure| panic!("{failure}: {text}"));
    assert_eq!(lowered.text.len(), text.len());
    lowered
}

fn tokens(text: &str) -> Vec<&str> {
    let arena = Allocator::default();
    let parsed = Parser::new(&arena, text, SourceType::mjs())
        .with_config(TokensParserConfig)
        .parse();
    assert!(parsed.diagnostics.is_empty());
    parsed
        .tokens
        .iter()
        .filter(|token| token.start() != token.end())
        .map(|token| &text[token.start() as usize..token.end() as usize])
        .collect()
}

fn compare(text: &str, expected: &str) {
    assert_eq!(tokens(&lower(text).text), tokens(expected), "{text}");
}

#[test]
fn declarations() {
    for text in [
        "import type {Model} from './missing.ts';",
        "import type Model from './missing.ts';",
        "import type * as Model from './missing.ts';",
        "export type {Model} from './missing.ts';",
        "export type * from './missing.ts';",
        "export type * as Model from './missing.ts';",
        "export type Model = number;",
        "export interface Model { value: number; }",
        "export default interface Model { value: number; }",
        "export declare class Model { value: number; }",
        "export declare const value: number;",
        "export declare function absent(): void;",
    ] {
        compare(text, ";");
    }
}

#[test]
fn imports() {
    for (text, expected) in [
        (
            "import {type Model, value} from './value.ts';",
            "import {value} from './value.ts';",
        ),
        (
            "import {value, type Model} from './value.ts';",
            "import {value} from './value.ts';",
        ),
        (
            "import {type Model, value, type Shape, other,} from './value.ts';",
            "import {value, other} from './value.ts';",
        ),
        (
            "import Default, {type Model, value} from './value.ts';",
            "import Default, {value} from './value.ts';",
        ),
        (
            "import Default, {type Model} from './value.ts';",
            "import Default, {} from './value.ts';",
        ),
        (
            "import {type Model, type Shape,} from './value.ts';",
            "import {} from './value.ts';",
        ),
    ] {
        compare(text, expected);
    }
    compare(
        "import Default, {type Model, value} from './value.ts' with {type: 'json'};",
        "import Default, {value} from './value.ts' with {type: 'json'};",
    );
}

#[test]
fn exports() {
    for (text, expected) in [
        (
            "export {type Model, value} from './value.ts';",
            "export {value} from './value.ts';",
        ),
        (
            "export {value, type Model} from './value.ts';",
            "export {value} from './value.ts';",
        ),
        (
            "export {type Model, type Shape,} from './value.ts';",
            "export {} from './value.ts';",
        ),
        (
            "const value = 7; export {type Model, value};",
            "const value = 7; export {value};",
        ),
        ("export const value: number = 7;", "export const value = 7;"),
        ("export default 7 as number;", "export default 7;"),
        (
            "export default class<T> { value: number = 7; }",
            "export default class { value = 7; }",
        ),
        (
            "export default function identity<T>(value: T): T { return value; }",
            "export default function identity(value) { return value; }",
        ),
    ] {
        compare(text, expected);
    }
}

#[test]
fn explicit() {
    for text in [
        "import {Model} from './model.ts'; const value: Model = 7;",
        "import './effects.ts';",
        "import Default, * as values from './values.ts'; export {Default, values};",
        "export * from './values.ts';",
        "export * as values from './values.ts';",
    ] {
        let lowered = lower(text);
        assert!(lowered.text.contains("from './model.ts'") || lowered.text == text);
    }
}

#[test]
fn names() {
    compare(
        "import {type Model, type as value} from './value.ts';",
        "import {type as value} from './value.ts';",
    );
    compare(
        "export {type Model, 'type' as value} from './value.ts';",
        "export {'type' as value} from './value.ts';",
    );
}

#[test]
fn positions() {
    let text = "import {type /*𐐀*/ Model,\r\nvalue, /*保持*/ type Shape} from './值.ts';";
    let lowered = lower(text);
    let offset = text.find("value").unwrap();
    assert_eq!(&lowered.text[offset..offset + 5], "value");
    let offset = text.find("'./值.ts'").unwrap();
    assert_eq!(
        &lowered.text[offset..offset + "'./值.ts'".len()],
        "'./值.ts'"
    );
    assert!(lowered.text.contains("\r\n"));
    assert!(lowered.text.contains("/*保持*/"));
}
