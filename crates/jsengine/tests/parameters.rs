use jsengine::{Addons, Engine, Failure, Fault, Invocation, Rejection, Value};

struct Host;

impl Addons for Host {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: Invocation,
    ) -> Result<Value, Failure> {
        match binding {
            0 => Err(Fault("host stopped".into()).into()),
            1 => engine.call(invocation.arguments[0].clone(), vec![], self),
            _ => panic!("unexpected native call"),
        }
    }
}

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Host)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn defaults() {
    for source in [
        "const f = (a = 7) => a; f() === 7 && f(undefined) === 7 && f(null) === null;",
        "const f = function (a = 7, b = a + 1) { return a + b; }; f() === 15 && f(1) === 3;",
        "const f = (a = 7, b) => b; f() === undefined && f(1, 2) === 2;",
        "let calls = 0; const f = (a = ++calls) => a; f(false) === false && f(0) === 0 && f('') === '' && calls === 0;",
        "const f = (a = {}) => a; f() !== f();",
        "const f = (a = '😀', b = a.length) => b; f() === 1;",
        "const f = (a = (x => x)(7)) => a; f() === 7;",
        "const f = (a = {read() { return 7; }}) => a.read(); f() === 7;",
        "const f = (a = () => 7) => a; f()() === 7 && f().name === 'a';",
        "const f = (a = function () {}) => a.name; f() === 'a';",
        "const f = (a = class {}) => a.name; f() === 'a';",
        "const f = (a = 7,) => a; f() === 7;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn ordering() {
    for source in [
        "let order = ''; const f = (a = (order += 'a'), b = (order += 'b')) => { order += 'c'; }; f(undefined, (order += 'r')); order === 'rac';",
        "let order = ''; const fail = () => { order += 'x'; throw 7; }; const f = (a = fail(), b = (order += 'b')) => { order += 'c'; }; try { f(); } catch (e) { e === 7 && order === 'x'; }",
        "let effects = 0; const f = (a = missing) => { try { effects = 1; } catch { effects = 2; } }; try { f(); } catch (e) { e.constructor === ReferenceError && effects === 0; }",
        "let effects = 0; const fail = () => { throw 7; }; const f = (a = ++effects) => a; try { f(fail()); } catch (e) { e === 7 && effects === 0; }",
        "let order = ''; class A { field = (order += 'f'); constructor(a = (order += 'p')) { order += 'c'; } } new A(); order === 'fpc';",
        "let order = ''; class A { constructor() { order += 'a'; } } class B extends A { field = (order += 'f'); constructor(a = (order += 'p')) { super(); order += 'c'; } } new B(); order === 'pafc';",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn environments() {
    for source in [
        "let outer = 1; const f = (read = () => outer) => { let outer = 2; return read; }; const read = f(); outer = 3; read() === 3;",
        "let outer = 1; const f = (a = outer) => { let outer = 2; return a; }; f() === 1;",
        "const f = (a = () => later) => { let later = 7; return a; }; const read = f(); try { read(); } catch (e) { e.constructor === ReferenceError; }",
        "const f = (a = 1, read = () => a) => { a = 2; return read; }; f()() === 2;",
        "const f = (read = () => later, later = 7) => read(); f() === 7;",
        "const f = (read = () => rest.length, ...rest) => read(); f(undefined, 1, 2) === 2;",
        "const f = (a = a) => a; try { f(); } catch (e) { e.constructor === ReferenceError; }",
        "let b = 7; const f = (a = b, b = 1) => a; try { f(); } catch (e) { e.constructor === ReferenceError; }",
        "const f = (a = (b = 1), b) => a; try { f(); } catch (e) { e.constructor === ReferenceError; }",
        "const f = (a = rest, ...rest) => a; try { f(); } catch (e) { e.constructor === ReferenceError; }",
        "let saved = null; const fail = () => { throw 7; }; const f = (a = (saved = () => b), b = fail()) => {}; try { f(); } catch {} try { saved(); } catch (e) { e.constructor === ReferenceError; }",
        "const f = (a = () => a) => a; const read = f(); read() === read;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn rest() {
    for source in [
        "const f = (...items) => items; f().length === 0 && f(1, 2).length === 2;",
        "const f = (first, ...items) => items; f(1).length === 0 && f(1, 2, 3)[0] === 2;",
        "const f = (first = 7, ...items) => first + items.length; f(undefined, 1, 2) === 9;",
        "const f = function (...items) { let sum = 0; for (const item of items) { sum += item; } return sum; }; f(1, 2, 3) === 6;",
        "const f = (...items) => items; f() !== f();",
        "const f = (first, ...items) => { items[0] = 7; return first; }; f(1, 2) === 1;",
        "const o = {n: 1}; const f = (...items) => { items[0].n = 7; }; f(o); o.n === 7;",
        "const f = (...items) => items; const items = f(undefined); items.length === 1 && items[0] === undefined;",
        "const f = (...items) => { items = [7]; return items[0]; }; f() === 7;",
        "const o = {f(a = 1, ...items) { return a + items.length; }}; o.f(undefined, 2, 3) === 3;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn contexts() {
    for source in [
        "const o = {value: 7, f(a = this.value) { return a; }}; o.f() === 7;",
        "const o = {value: 7, make() { return (a = this.value) => a; }}; const f = o.make(); f() === 7;",
        "const F = function (a = new.target) { this.target = a; }; new F().target === F;",
        "const F = function (a = this) { this.self = a; }; const f = new F(); f.self === f;",
        "class A { read() { return this.value; } } class B extends A { value = 7; f(a = super.read()) { return a; } } new B().f() === 7;",
        "class A { read() { return this.value; } } class B extends A { value = 7; f = (a = super.read()) => a; } const b = new B(); const f = b.f; f() === 7;",
        "class A {} class B extends A { constructor(a = this) { super(); } } try { new B(); } catch (e) { e.constructor === ReferenceError; }",
        "class A {} class B extends A { constructor(a = super()) { this.self = a; } } const b = new B(); b.self === b;",
        "class A {} class B extends A { constructor(read = () => this) { super(); this.read = read; } } const b = new B(); b.read() === b;",
        "class A { constructor(...items) { this.items = items; } } class B extends A {} const b = new B(1, 2); b.items.length === 2;",
        "class A { static f(a = this) { return a; } } class B extends A {} B.f() === B;",
        "const o = {stored: 0, set value(a = 7) { this.stored = a; }}; o.value = undefined; o.stored === 7;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn metadata() {
    for source in [
        "((a, b = 1, c) => {}).length === 1;",
        "((...items) => {}).length === 0 && ((a, b, ...items) => {}).length === 2;",
        "(function (a = 7) {}).length === 0;",
        "class A { constructor(a, b = 1, ...items) {} read(a = 1) {} } A.length === 1 && A.prototype.read.length === 0;",
        "class A { constructor(a, ...items) {} } class B extends A {} A.length === 1 && B.length === 0;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    let mut engine = Engine::default();
    let object = engine
        .execute("({set value(a = 1) {}});", &mut Host)
        .unwrap();
    let jsengine::Content::Accessor { set: Some(set), .. } = engine
        .descriptor(&object, "value")
        .unwrap()
        .unwrap()
        .content
    else {
        panic!("setter descriptor")
    };
    assert_eq!(engine.get(&set, "length").unwrap(), Value::Number(0.0));
}

#[test]
fn syntax() {
    for source in [
        "(a, a = 1) => 0;",
        "(a, ...a) => 0;",
        "(...items,) => 0;",
        "(...items = []) => 0;",
        "(...items, a) => 0;",
        "(a = 1) => { let a = 2; };",
        "(...items) => { const items = []; };",
        "({get value(a = 1) {}});",
        "({set value(...items) {}});",
        "(a = new.target) => a;",
        "(a = super()) => a;",
        "class A { field = (a = arguments) => a; }",
        "class A { method() { return function (a = super.value) {}; } }",
        "const f = function (...items,) {};",
        "const f = function (a = ) {};",
        "(a = 1)\n=> a;",
    ] {
        let source = format!("effects = 1; {source}");
        assert!(
            matches!(Engine::check(&source), Err(Rejection::Syntax(_))),
            "{source}"
        );
        let mut engine = Engine::default();
        engine.execute("let effects = 0;", &mut Host).unwrap();
        assert!(matches!(
            engine.execute(&source, &mut Host),
            Err(Failure::Throw(_))
        ));
        assert_eq!(
            engine.execute("effects;", &mut Host).unwrap(),
            Value::Number(0.0)
        );
    }
}

#[test]
fn boundaries() {
    let mut engine = Engine::default();
    let stop = engine.native(0);
    engine.register("stop", stop).unwrap();
    let call = engine.native(1);
    engine.register("host", call).unwrap();
    engine
        .execute(
            "let effects = 0; const f = (a = stop()) => { effects = 1; };",
            &mut Host,
        )
        .unwrap();
    assert!(matches!(
        engine.execute(
            "try { f(); } catch { effects = 2; } finally { effects = 3; }",
            &mut Host
        ),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("effects;", &mut Host).unwrap(),
        Value::Number(0.0)
    );
    let f = engine
        .execute(
            "(a = (effects = 1), ...items) => { effects = 2; };",
            &mut Host,
        )
        .unwrap();
    let foreign = Engine::default().object(Default::default()).unwrap();
    assert!(matches!(
        engine.call(f.clone(), vec![Value::Undefined, foreign], &mut Host),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("effects;", &mut Host).unwrap(),
        Value::Number(0.0)
    );
    assert!(matches!(
        engine.call(f, vec![Value::Undefined; 4098], &mut Host),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("effects;", &mut Host).unwrap(),
        Value::Number(1.0)
    );
    assert!(matches!(
        engine.execute(
            "const recurse = (a = host(recurse)) => 1; recurse();",
            &mut Host
        ),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("((a = 7) => a)();", &mut Host).unwrap(),
        Value::Number(7.0)
    );
}
