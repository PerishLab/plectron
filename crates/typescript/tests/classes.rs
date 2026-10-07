use jsengine::{Addons, Engine, Failure, Invocation, Value};
use project::{Kind, Module, Source};

struct Bare;

impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}

fn lower(text: &str) -> typescript::Lowered {
    let source = Source {
        module: Module::new("class.ts").unwrap(),
        kind: Kind::Ts,
        text: text.into(),
    };
    let lowered = typescript::erase(&source).unwrap_or_else(|failure| panic!("{failure}: {text}"));
    assert_eq!(lowered.text.len(), text.len());
    lowered
}

fn execute(text: &str) {
    let lowered = lower(text);
    let mut engine = Engine::default();
    let result = engine
        .execute(&lowered.text, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {}", engine.diagnostic(&failure), lowered.text));
    assert_eq!(result, Value::Number(7.0), "{text}");
}

#[test]
fn fields() {
    for text in [
        "class Model<T> { public readonly value: number = 7; } new Model<number>().value;",
        "class Model { private value!: number; constructor() { this.value = 7; } } new Model().value;",
        "class Model { protected value?: number = 7; } new Model().value;",
        "class Model { readonly ['value']: number = 7; } new Model().value;",
        "class Model { public public: number = 7; } new Model().public;",
        "class Model { static readonly value: number = 7; } Model.value;",
        "class Base { value = 1; } class Model extends Base { override value: number = 7; } new Model().value;",
    ] {
        execute(text);
    }
}

#[test]
fn members() {
    for text in [
        "class Model { public value<T>(number: T): T { return number; } } new Model().value<number>(7);",
        "class Model { protected get value(): number { return 7; } } new Model().value;",
        "class Model { private number = 0; public set value(number: number) { this.number = number; } public get value(): number { return this.number; } } const model = new Model(); model.value = 7; model.value;",
        "class Model { value(number: string): number; value(number: number): number; value(number: number): number { return number; } } new Model().value(7);",
        "class Model { value?(number: number): number { return number; } } new Model().value(7);",
    ] {
        execute(text);
    }
}

#[test]
fn absence() {
    for text in [
        "declare class Absent { value: number; } let result = 0; if (typeof Absent === 'undefined') { result = 7; } result;",
        "class Model { declare absent: number; value = 7; } const model = new Model(); let result = 0; if (!('absent' in model)) { result = model.value; } result;",
        "abstract class Model { abstract absent: number; abstract method(): void; value = 7; } const model = new Model(); let result = 0; if (!('absent' in model || 'method' in model)) { result = model.value; } result;",
        "class Model { [key: string]: number; value = 7; } new Model().value;",
    ] {
        execute(text);
    }
}

#[test]
fn heritage() {
    execute(
        "interface Shape<T> { value: T; } class Base<T> {} class Model<T> extends Base<T> implements Shape<T> { value: number = 7; } new Model<number>().value;",
    );
    execute(
        "const Model = class<T> implements Shape<T> { value = 7; }; new Model<number>().value;",
    );
}

#[test]
fn termination() {
    for text in [
        "class Model { value:\nnumber\nmethod(): number { return 7; } } new Model().method();",
        "class Model { value = 7 as\nnumber\nmethod(): number { return this.value; } } new Model().method();",
        "class Model { value!:\nnumber\n['method'](): number { return 7; } } new Model().method();",
    ] {
        execute(text);
    }
}

#[test]
fn keywords() {
    let text = "class Model { public public = 7; readonly readonly = 7; static static = 7; async async(): Promise<number> { return 7; } get get(): number { return 7; } }";
    let lowered = lower(text);
    assert!(lowered.text.contains("public = 7"));
    assert!(lowered.text.contains("readonly = 7"));
    assert!(lowered.text.contains("static static = 7"));
    assert!(lowered.text.contains("async async()"));
    assert!(lowered.text.contains("get get()"));
}

#[test]
fn positions() {
    let text = "abstract /*𐐀*/ class Model<T> implements Shape<T> {\r\npublic /*保留*/ value: number = 7;\n}";
    let lowered = lower(text);
    for (offset, character) in text.char_indices() {
        if matches!(character, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
            assert_eq!(
                &lowered.text[offset..offset + character.len_utf8()],
                character.to_string()
            );
        }
    }
    let start = text.find("value").unwrap();
    assert_eq!(&lowered.text[start..start + 5], "value");
    assert!(lowered.text.contains("/*𐐀*/"));
    assert!(lowered.text.contains("/*保留*/"));
}

#[test]
fn unchanged() {
    let text = "class Model { value = 7; method() { return this.value; } } new Model().method();";
    let lowered = lower(text);
    assert_eq!(lowered.text, text);
    assert!(lowered.repairs.is_empty());
}
