use crate::native::Bare;
use jsengine::{Engine, Failure, Rejection, Value};

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn arithmetic() {
    for source in [
        "let n = 2; n += 3; n *= 4; n -= 2; n /= 3; n %= 4; n === 2;",
        "let x = 1; let y = 2; x += y *= 3; x === 7 && y === 6;",
        "let text = 'a'; text += 7; text === 'a7';",
        "let n = '7'; n -= 2; n === 5;",
        "let calls = 0; let order = ''; const o = {n: 3, get value() { order += 'g'; return this.n; }, set value(n) { order += 's'; this.n = n; }}; const base = function () { calls++; order += 'b'; return o; }; const key = function () { calls++; order += 'k'; return 'value'; }; const right = function () { calls++; order += 'r'; return 4; }; const result = (base()[key()] += right()); result === 7 && o.n === 7 && calls === 3 && order === 'bkgrs';",
        "let n = 1; n += (n = 7); n === 8;",
        "let effects = 0; try { missing += (effects = 1); } catch (e) { effects === 0 && e.constructor === ReferenceError; }",
        "let effects = 0; try { null.value += (effects = 1); } catch (e) { effects === 0 && e.constructor === TypeError; }",
        "let effects = 0; const o = {get value() { throw 7; }}; try { o.value += (effects = 1); } catch (e) { effects === 0 && e === 7; }",
        "let effects = 0; const n = 1; try { n += (effects = 7); } catch (e) { effects === 7 && n === 1 && e.constructor === TypeError; }",
        "class A { get value() { return this.n; } set value(n) { this.n = n; } } class B extends A { n = 1; update() { return super.value += 2; } } const b = new B(); b.update() === 3 && b.n === 3 && A.prototype.n === undefined;",
        "const values = [1]; values.length += 2; values[2] = 7; values.length === 3 && values[1] === undefined;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn logical() {
    for source in [
        "let a = 0; let b = 1; let c = null; a ||= 2; b &&= 3; c ??= 4; a === 2 && b === 3 && c === 4;",
        "const a = 0; const b = 1; const c = ''; a &&= missing; b ||= missing; c ??= missing; a === 0 && b === 1 && c === '';",
        "let a = false; let b = 0; a ??= missing; b ??= missing; a === false && b === 0;",
        "let n = 0 / 0; n ??= missing; n !== n;",
        "let writes = 0; let reads = 0; const o = {get value() { reads++; return 7; }, set value(n) { writes++; }}; const result = (o.value ||= missing); reads === 1 && writes === 0 && result === 7;",
        "let writes = 0; let reads = 0; const o = {get value() { reads++; return 0; }, set value(n) { writes++; }}; const result = (o.value &&= missing); reads === 1 && writes === 0 && result === 0;",
        "let order = ''; const o = {get value() { order += 'g'; return undefined; }, set value(n) { order += 's'; }}; const result = (o.value ??= (order += 'r')); result === 'gr' && order === 'grs';",
        "let C = undefined; C ||= class {}; let fn = null; fn ??= function () {}; C.name === 'C' && fn.name === 'fn';",
        "const o = {}; o.C ||= class {}; o.C.name === '';",
        "let a = 0; let b = 1; a ||= b &&= 7; a === 7 && b === 7;",
        "const o = {get value() { return 1; }}; o.value ||= missing; o.value === 1;",
        "const o = {get value() { return 0; }}; try { o.value ||= 7; } catch (e) { e.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn updates() {
    for source in [
        "let n = 1; const a = n++; const b = ++n; const c = n--; const d = --n; a === 1 && b === 3 && c === 3 && d === 1 && n === 1;",
        "let n = '2'; const old = n++; old === 2 && n === 3;",
        "let n = null; const value = ++n; value === 1 && n === 1;",
        "let n = undefined; const old = n++; old !== old && n !== n;",
        "let n = -0; const old = n++; 1 / old === -1 / 0 && n === 1;",
        "let x = 1; let y = 2; const result = x+++y; result === 3 && x === 2 && y === 2;",
        "let x = 1; let y = 2; x;\n++y; y === 3 && x === 1;",
        "let calls = 0; let order = ''; const o = {n: 1, get value() { order += 'g'; return this.n; }, set value(n) { order += 's'; this.n = n; }}; const base = function () { calls++; return o; }; const key = function () { calls++; return 'value'; }; const old = base()[key()]++; old === 1 && o.n === 2 && calls === 2 && order === 'gs';",
        "let reads = 0; const o = {get value() { reads++; return 1; }, set value(n) { throw n; }}; try { o.value++; } catch (e) { reads === 1 && e === 2; }",
        "class A { get value() { return this.n; } set value(n) { this.n = n; } } class B extends A { n = 1; update() { return ++super.value; } } const b = new B(); b.update() === 2 && b.n === 2;",
        "const n = 1; try { n++; } catch (e) { n === 1 && e.constructor === TypeError; }",
        "const a = [1]; a.length++; a.length === 2 && a[1] === undefined;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn syntax() {
    for source in [
        "1 += 2;",
        "(a + b) *= 2;",
        "eval ||= 1;",
        "arguments++;",
        "++eval;",
        "++(1);",
        "new C()++;",
        "n++++;",
        "++n++;",
        "let n = 1; n\n++;",
        "let n = 1; n/*\n*/++;",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
}

#[test]
fn conversion() {
    for expression in [
        "n++",
        "--n",
        "n += 1",
        "n -= 1",
        "n *= 1",
        "n /= 1",
        "n %= 1",
        "+n",
        "-n",
        "'text' + n",
        "n < 1",
        "(a.length = n)",
    ] {
        let mut engine = Engine::default();
        engine
            .execute(
                "let effects = 0; const a = []; let n = {valueOf() { effects++; return 7; }};",
                &mut Bare,
            )
            .unwrap();
        let source = format!(
            "try {{ {expression}; }} catch {{ effects = 10; }} finally {{ effects = 20; }}"
        );
        let failure = engine.execute(&source, &mut Bare).unwrap_err();
        assert!(matches!(failure, Failure::Fault(_)), "{expression}");
        assert!(
            failure
                .to_string()
                .contains("object coercion is unsupported")
        );
        assert_eq!(
            engine.execute("effects;", &mut Bare).unwrap(),
            Value::Number(0.0)
        );
    }
}
#[test]
fn references() {
    for source in [
        "let key = 'x'; let reads = 0; const a = {x: 1}; const b = {x: 10}; let current = a; const base = () => { reads++; return current; }; const rhs = () => { current = b; key = 'y'; return 2; }; base()[key] += rhs(); reads === 1 && a.x === 3 && b.x === 10 && a.y === undefined;",
        "let order = ''; const a = {get method() { order += 'g'; return function(n) { order += 'c'; return this === a && n === 2; }; }}; const arg = () => { order += 'a'; return 2; }; const result = a.method(arg()); result && order === 'gac';",
        "let effects = 0; try { null.value = (effects = 1); } catch (e) { effects === 1 && e.constructor === TypeError; }",
        "class A { get value() { return this.n; } set value(n) { this.n = n; } } class B extends A { n = 3; run() { return super.value += 4; } } const b = new B(); b.run() === 7 && b.n === 7;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}
