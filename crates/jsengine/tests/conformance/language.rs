use super::native::Bare;
use jsengine::{Engine, Value};

fn evaluate(source: &str) -> Value {
    let mut host = Bare;
    Engine::default().execute(source, &mut host).unwrap()
}

fn reject(source: &str, message: &str) {
    let mut host = Bare;
    let mut engine = Engine::default();
    let failure = engine.execute(source, &mut host).unwrap_err();
    let error = engine.diagnostic(&failure);
    assert!(error.0.contains(message), "{error}");
}

#[test]
fn arithmetic() {
    assert_eq!(evaluate("2 + 3 * 4;"), Value::Number(14.0));
    assert_eq!(evaluate("(2 + 3) * 4;"), Value::Number(20.0));
    assert_eq!(evaluate("-5 + 9 % 4;"), Value::Number(-4.0));
}

#[test]
fn primitives() {
    assert_eq!(evaluate("'clicks: ' + 2;"), Value::Text("clicks: 2".into()));
    assert_eq!(evaluate("null + true;"), Value::Number(1.0));
    assert_eq!(evaluate("1 === '1';"), Value::Boolean(false));
    assert_eq!(evaluate("0 / 0 === 0 / 0;"), Value::Boolean(false));
    assert_eq!(evaluate("'abc' < 'abd';"), Value::Boolean(true));
    assert_eq!(evaluate("'汉字';"), Value::Text("汉字".into()));
}

#[test]
fn typeofvalues() {
    for source in [
        "typeof undefined === 'undefined' && typeof missing === 'undefined';",
        "typeof null === 'object' && typeof true === 'boolean' && typeof 2 === 'number';",
        "typeof 'text' === 'string' && typeof Symbol() === 'symbol';",
        "typeof {} === 'object' && typeof [] === 'object' && typeof (() => 1) === 'function';",
        "typeof new Proxy(() => 1, {}) === 'function';",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    reject("typeof missing; let missing = 1;", "not initialized");
}

#[test]
fn shorting() {
    assert_eq!(evaluate("false && absent();"), Value::Boolean(false));
    assert_eq!(evaluate("'held' || absent();"), Value::Text("held".into()));
    assert_eq!(
        evaluate("let x = 0; true || (x = 1); x;"),
        Value::Number(0.0)
    );
}

#[test]
fn scope() {
    assert_eq!(
        evaluate("let x = 1; { let x = 2; x = 3; } x;"),
        Value::Number(1.0)
    );
    assert_eq!(evaluate("let x = 1; { x = 3; } x;"), Value::Number(3.0));
    reject("const x = 1; x = 2;", "const");
    reject("let x = 1; let x = 2;", "duplicate");
    reject("let x = 1; { x; let x = 2; }", "not initialized");
    reject("let x = x;", "not initialized");
}

#[test]
fn closures() {
    let source = "const make = function (start) { let n = start; return function () { n = n + 1; return n; }; }; const a = make(0); const b = make(10); a(); b(); a();";
    assert_eq!(evaluate(source), Value::Number(2.0));
    assert_eq!(
        evaluate("const f = function (x) { return x; }; f();"),
        Value::Undefined
    );
    assert_eq!(
        evaluate("const f = function () { 42; }; f();"),
        Value::Undefined
    );
}

#[test]
fn recursion() {
    let source =
        "const fact = function (n) { if (n <= 1) { return 1; } return n * fact(n - 1); }; fact(6);";
    assert_eq!(evaluate(source), Value::Number(720.0));
    reject("const f = function () { return f(); }; f();", "depth");
}

#[test]
fn objects() {
    assert_eq!(
        evaluate("const p = { x: 3, nested: { y: 4 } }; p.x + p.nested.y;"),
        Value::Number(7.0)
    );
    assert_eq!(evaluate("const p = {}; p.missing;"), Value::Undefined);
    assert_eq!(evaluate("const p = {}; p === p;"), Value::Boolean(true));
    assert_eq!(evaluate("({}) === ({});"), Value::Boolean(false));
}

#[test]
fn errors() {
    reject("missing;", "unknown binding");
    reject("1();", "not callable");
    reject("null.x;", "null or undefined");
    reject("return 1;", "return outside");
    reject("const f = function (x, x) {};", "duplicate parameter");
    reject("var x = 1;", "unsupported keyword");
    assert_eq!(evaluate("const f = () => 1; f();"), Value::Number(1.0));
    reject("const x = 1", "expected ;");
    reject("'unterminated", "unterminated");
    reject("01;", "leading-zero");
}

#[test]
fn parsing() {
    reject("missing(); while (true) {}", "unsupported keyword");
}

#[test]
fn nesting() {
    reject(
        &format!("{}1{};", "(".repeat(100), ")".repeat(100)),
        "nesting",
    );
    reject(&" ".repeat(65_537), "64 KiB");
}

#[test]
fn declarations() {
    reject(
        "missing(); if (false) { let x = 1; let x = 2; }",
        "duplicate",
    );

    reject("const f = function (x) { let x = 2; };", "duplicate");
    reject("const f = function () { return\n1; };", "same line");
    reject("const x = { __proto__: null };", "prototype");
}

#[test]
fn strings() {
    let source = "const double = function (n, s) { if (n === 0) { return s; } return double(n - 1, s + s); }; double(21, 'x');";
    reject(source, "string budget");
}

#[test]
fn scanning() {
    assert_eq!(
        evaluate("/*雪*/\u{2003}let $x_2=12.; // 🦀\n$x_2 !== 3 && $x_2 >= 12;"),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(r#"'雪\n\t\r\\\"\'';"#),
        Value::Text("雪\n\t\r\\\"'".into())
    );
    assert_eq!(
        evaluate("const f = function () { return /*雪*/ 3; }; f();"),
        Value::Number(3.0)
    );
    reject("const f = function () { return /*雪\n*/ 3; };", "same line");
    reject("'雪'; @;", "character 5");
    reject("/*雪", "unterminated comment");
    assert_eq!(evaluate(r#"'\u1234';"#), Value::Text("ሴ".into()));
    reject("'雪\n';", "newline in string");
    reject("1 == 1;", "unsupported syntax");
    reject("0x10;", "expected ;");
}

#[test]
fn addresses() {
    let mut engine = Engine::default();
    engine.execute("let target = 1; const callbacks = []; for (let value of [1, 2, 3]) { callbacks[callbacks.length] = () => { value += 1; return value; }; } const churn = () => { for (const a of [0,1,2,3,4,5,6,7]) { for (const b of [0,1,2,3,4,5,6,7]) { let local = a + b; } } return 5; }; target += churn();", &mut Bare).unwrap();
    assert_eq!(
        engine.execute("target;", &mut Bare).unwrap(),
        Value::Number(6.0)
    );
    assert_eq!(
        engine
            .execute(
                "callbacks[0]() + callbacks[0]() + callbacks[2]();",
                &mut Bare
            )
            .unwrap(),
        Value::Number(9.0)
    );
    reject(
        "let outer = 1; { outer = 2; let outer = 3; }",
        "not initialized",
    );
    reject(
        "const fixed = 1; const rhs = () => { let fixed = 2; return fixed; }; fixed = rhs();",
        "const binding",
    );
}
