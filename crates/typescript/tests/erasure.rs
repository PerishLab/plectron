use jsengine::{Addons, Engine, Failure, Invocation, Value};
use project::{Kind, Module, Source};

struct Bare;

impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}

fn source(text: &str) -> Source {
    Source {
        module: Module::new("main.ts").unwrap(),
        kind: Kind::Ts,
        text: text.into(),
    }
}

fn execute(text: &str) -> Value {
    let lowered =
        typescript::erase(&source(text)).unwrap_or_else(|error| panic!("{error}: {text}"));
    assert_eq!(lowered.text.len(), text.len());
    let mut engine = Engine::default();
    engine
        .execute(&lowered.text, &mut Bare)
        .unwrap_or_else(|error| panic!("{}: {}", engine.diagnostic(&error), lowered.text))
}

#[test]
fn annotations() {
    for text in [
        "const value: number = 7; value;",
        "const identity = <T>(value: T): T => value; identity<number>(7);",
        "const pick = function(first?: number, ...rest: number[]): number { return first! + rest[0]; }; pick(3, 4);",
        "const pick = function(this: unknown, value: number): number { return value; }; pick(7);",
        "const pick = function(this: unknown): number { return 7; }; pick();",
    ] {
        assert_eq!(execute(text), Value::Number(7.0), "{text}");
    }
}

#[test]
fn assertions() {
    for text in [
        "const value = (7 as number) satisfies number; value!;",
        "const value: number = 7; (value as unknown as number)!;",
        "const identity = (value: number) => value; (identity<number>)(7);",
        "const value = {number: 7}; value!.number;",
        "const value = [7]; value![0];",
    ] {
        assert_eq!(execute(text), Value::Number(7.0), "{text}");
    }
}

#[test]
fn declarations() {
    let text = "interface Model { value: number; } type Pick<T> = T extends unknown ? T : never; declare const ambient: number; declare function absent(value: number): void; const identity = (value: number): number => value; identity(7);";
    assert_eq!(execute(text), Value::Number(7.0));
}

#[test]
fn arrows() {
    for gap in [
        "\n",
        "\r\n",
        "\u{2028}",
        "\u{2029}",
        " /*\n*/",
        " // erased type\n",
    ] {
        let text = format!("const pick = (value: number){gap}: number => value; pick(7);");
        let lowered = typescript::erase(&source(&text)).unwrap();
        let repair = lowered
            .repairs
            .iter()
            .find(|repair| &lowered.text[repair.generated.start..repair.generated.end] == ")")
            .unwrap();
        assert_eq!(&text[repair.source.start..repair.source.end], ")");
        assert_eq!(execute(&text), Value::Number(7.0));
    }
    for text in [
        "const pick = (value: number):\nnumber => value; pick(7);",
        "const pick = (value: number): /*\n*/ number => value; pick(7);",
        "const pick = (value: number): {\nvalue: number\n} => ({value: value}); pick(7).value;",
        "const pick = (value: number):\n类型 => value; pick(7);",
    ] {
        assert_eq!(execute(text), Value::Number(7.0), "{text}");
    }
}

#[test]
fn termination() {
    for text in [
        "let result = 0; const value = 1 as number\n(result = 7); result;",
        "let result = 0; 1 as number\n(result = 7); result;",
        "let result = 0; 1 satisfies number\n(result = 7); result;",
        "const pick = () => { return 7 as number\n}; pick();",
        "const pick = <T>(value: T) => value; const other = pick<number>\nlet result = other(7); result;",
        "let result = 0; const pick = () => 1; pick()\ntype Model = number;\n(result = 7); result;",
        "let result = 0; const pick = () => 1; pick()\ninterface Model {}\n(result = 7); result;",
        "let result = 0; const pick = () => 1; pick()\ndeclare const missing: number;\n(result = 7); result;",
    ] {
        assert_eq!(execute(text), Value::Number(7.0), "{text}");
    }
}

#[test]
fn loops() {
    for text in [
        "let result = 0; for (let count: number = 0; count < 7; count++) { result++; } result;",
        "let result = 0; for (let count = 0 as number; count < 7; count++) { result++; } result;",
        "let result = 0; for (const value of [7] as number[]) { result += value; } result;",
        "let result = 0; for (const value in {one: 7} as object) { result += 7; } result;",
    ] {
        let lowered = typescript::erase(&source(text)).unwrap();
        assert!(lowered.repairs.is_empty());
    }
    assert_eq!(
        execute(
            "let result = 0; for (const value of [7] as number[]) { result += value; } result;"
        ),
        Value::Number(7.0)
    );
    assert!(typescript::erase(&source("for (const value: number of [7]) {} ")).is_err());
}

#[test]
fn grammar() {
    for text in [
        "function identity<T>(value: T): T { return value; } identity<number>(7);",
        "const {value}: {value: number} = {value: 7}; value;",
        "function pick({value}: {value: number}): number { return value; } pick({value: 7});",
        "let value!: number; value = 7; value;",
        "function identity(value: number): number; function identity(value: number): number { return value; } identity(7);",
        "const pattern: RegExp = /as|satisfies|!/; pattern.test('as') ? 7 : 0;",
    ] {
        let lowered = typescript::erase(&source(text)).unwrap();
        assert_eq!(lowered.text.len(), text.len());
        assert!(
            Engine::check(&lowered.text).is_err(),
            "new JS capability must be verified: {text}"
        );
    }
}

#[test]
fn positions() {
    let text = "type 名字 = '𐐀';\r\ninterface Model {\u{2028}value: number\u{2029}}\nconst message: 名字 = '𐐀';\nmessage;";
    let lowered = typescript::erase(&source(text)).unwrap();
    assert_eq!(lowered.text.len(), text.len());
    for (offset, character) in text.char_indices() {
        if matches!(character, '\r' | '\n' | '\u{2028}' | '\u{2029}') {
            assert_eq!(
                &lowered.text[offset..offset + character.len_utf8()],
                character.to_string()
            );
        }
    }
    let offset = text.find("const message").unwrap();
    assert_eq!(&lowered.text[offset..offset + 13], "const message");
    assert_eq!(execute(text), Value::Text("𐐀".into()));
}

#[test]
fn unchanged() {
    let text = "const text = 'as satisfies enum interface'; // unchanged\nconst value = `value: ${text}`; value;";
    let lowered = typescript::erase(&source(text)).unwrap();
    assert_eq!(lowered.text, text);
    assert!(lowered.repairs.is_empty());
}

#[test]
fn bindings() {
    for text in [
        "type Model = number; export {Model};",
        "interface Model {} export {Model};",
        "declare const value: number; export {value};",
    ] {
        typescript::check(&source(text)).unwrap();
        let failure = typescript::erase(&source(text)).unwrap_err();
        assert!(failure.message.contains("unverified lowering"));
        assert_eq!(failure.module, source(text).module);
    }
}

#[test]
fn excluded() {
    for text in [
        "effect(); enum Model { One }",
        "effect(); const value = ;",
        "effect(); namespace Model {}",
    ] {
        assert_eq!(
            typescript::erase(&source(text)).unwrap_err(),
            typescript::check(&source(text)).unwrap_err()
        );
    }
}
