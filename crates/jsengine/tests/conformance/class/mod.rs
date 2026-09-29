mod accessor;
mod derived;
mod fields;
mod inspection;
mod parent;

use crate::native::Bare;
use jsengine::{Engine, Failure, Invocation, Rejection, Value};

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn construction() {
    for source in [
        "class C {} const a = new C(); a instanceof C && a.constructor === C;",
        "class C { constructor(n) { this.n = n; } read() { return this.n; } } const a = new C(7); a.read() === 7 && a instanceof C;",
        "const C = class Named { constructor() { this.type = Named; } }; new C().type === C;",
        "const result = {}; class C { constructor() { return result; } } new C() === result;",
        "class C { constructor() { this.value = 7; return 0; } } new C().value === 7;",
        "class C { constructor() { try { return {}; } finally { return null; } } } new C() instanceof C;",
        "class C { constructor() { throw 7; } } try { new C(); } catch (error) { error === 7; }",
        "let count = 0; class C { constructor() { count = 10; } } try { C(count = 1); } catch (error) { count === 1 && error.constructor === TypeError; }",
        "class C {} try { C.prototype = {}; } catch (error) { error.constructor === TypeError; }",
        "class C { ['constructor']() { return 7; } } const a = new C(); a.constructor() === 7 && a instanceof C;",
        "class C { static constructor() { return 7; } } C.constructor() === 7 && new C().constructor === C;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn methods() {
    for source in [
        "class C { method() { return this; } static method() { return this; } } const a = new C(); a.method() === a && C.method() === C;",
        "class C { method() { return this; } } const method = new C().method; method() === undefined;",
        "class C { method() {} static other() {} } C.prototype.method.prototype === undefined && C.other.prototype === undefined;",
        "class C { method() {} } try { new C.prototype.method(); } catch (error) { error.constructor === TypeError; }",
        "class C { static method() {} } try { new C.method(); } catch (error) { error.constructor === TypeError; }",
        "class C { method() { return 1; } method() { return 2; } } new C().method() === 2;",
        "class C { method() { return 1; } } const a = new C(); a.method = function () { return 2; }; const b = new C(); a.method() === 2 && b.method() === 1;",
        "class C { static() { return 1; } static static() { return 2; } } new C().static() === 1 && C.static() === 2;",
        "class C { 'value'() { return 1; } 2() { return 2; } ['v' + 'alue']() { return 3; } } const a = new C(); a.value() === 3 && a[2]() === 2;",
        "class C { __proto__() { return 1; } } new C().__proto__() === 1;",
        "const make = function (n) { return class { read() { return n; } }; }; const A = make(1); const B = make(2); new A().read() === 1 && new B().read() === 2;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn bindings() {
    for source in [
        "try { new C(); class C {} } catch (error) { error.constructor === ReferenceError; }",
        "class C { static self() { return C; } } const original = C; C = 7; original.self() === original;",
        "class C { static change() { C = 1; } } try { C.change(); } catch (error) { error.constructor === TypeError; }",
        "const C = class Named { static self() { return Named; } }; C.self() === C;",
        "const C = class Named {}; try { Named; } catch (error) { error.constructor === ReferenceError; }",
        "let C = class { static self() { return C; } }; const old = C; C = 7; old.self() === 7;",
        "let saved = undefined; { class C { static self() { return C; } } saved = C; } saved.self() === saved;",
        "let saved = []; for (let n of [0, 1]) { class C { read() { return n; } } saved[n] = new C(); } saved[0].read() === 0 && saved[1].read() === 1;",
        "class C { static method(C) { C = 7; return C; } } C.method() === 7;",
        "class C { method() { class C {} return C; } } new C().method() !== C;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn computation() {
    for source in [
        "let order = ''; const key = function (n) { order = order + n; return n; }; class C { [key('a')]() {} constructor() {} static [key('b')]() {} [key('a')]() {} } new C(); new C(); order === 'aba';",
        "try { class C { [C]() {} } } catch (error) { error.constructor === ReferenceError; }",
        "try { const C = class { [C]() {} }; } catch (error) { error.constructor === ReferenceError; }",
        "let saved = undefined; class C { [(saved = function () { return C; }) && 'm']() {} } saved() === C;",
        "let saved = undefined; try { class C { [(saved = function () { return C; }) && 'm']() {} [missing]() {} } } catch {} try { saved(); } catch (error) { error.constructor === ReferenceError; }",
        "let marker = 0; const stop = function () { throw 7; }; try { class C { [stop()]() {} [(marker = 1)]() {} } } catch (error) { marker === 0 && error === 7; }",
        "const holder = {make: function () { return class { [this.key]() { return 7; } }; }, key: 'value'}; const C = holder.make(); new C().value() === 7;",
        "let marker = 0; try { class C { static ['prototype']() {} [(marker = 1)]() {} } } catch (error) { marker === 0 && error.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn names() {
    for source in [
        "class C { constructor(a, b) {} read(a) {} static write(a, b, c) {} } C.name === 'C' && C.length === 2 && C.prototype.read.name === 'read' && C.prototype.read.length === 1 && C.write.length === 3;",
        "const C = class {}; C.name === 'C' && C.length === 0;",
        "const C = class Named {}; C.name === 'Named';",
        "const holder = {C: class {}}; holder.C.name === 'C';",
        "let C = undefined; C = class {}; C.name === 'C';",
        "const C = class { static name() { return 'method'; } }; C.name() === 'method';",
        "const C = class { static length() { return 7; } }; C.length() === 7;",
        "const holder = {}; holder.C = class {}; holder.C.name === '';",
        "const C = class { ['']() {} [7]() {} }; C.prototype[''].name === '' && C.prototype[7].name === '7';",
        "const fn = function (a) {}; fn.name === 'fn' && fn.length === 1;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn rejection() {
    for source in [
        "class {}",
        "class C { constructor() {} constructor() {} }",
        "class C { constructor() {} 'constructor'() {} }",
        "class C { static prototype() {} }",
        "class C { static 'prototype'() {} }",
        "class C { m(a, a) {} }",
        "class C { m(eval) {} }",
        "class C {} let C = 1;",
        "let C = 1; class C {}",
        "class C { m() { class X {} const X = 1; } }",
        "if (true) class C {}",
        "for (let n of [1]) class C {}",
        "class C { m() { break; } }",
        "for (let n of [1]) { class C { m() { continue; } } }",
        "try {} catch (C) { class C {} }",
        "class eval {}",
        "class arguments {}",
        "class C { static {} }",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
    let mut engine = Engine::default();
    engine.execute("let marker = 0;", &mut Bare).unwrap();
    assert!(
        engine
            .execute(
                "marker = 1; class C { constructor() {} constructor() {} }",
                &mut Bare
            )
            .is_err()
    );
    assert_eq!(
        engine.execute("marker;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
    engine.execute("class C {}", &mut Bare).unwrap();
    assert!(engine.execute("marker = 1; class C {}", &mut Bare).is_err());
    assert_eq!(
        engine.execute("marker;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
}

#[test]
fn host() {
    let mut engine = Engine::default();
    let class = engine
        .execute(
            "class C { constructor(n) { this.n = n; } value() { return this; } } C;",
            &mut Bare,
        )
        .unwrap();
    let value = engine
        .construct(class.clone(), vec![Value::Number(7.0)], &mut Bare)
        .unwrap();
    assert_eq!(engine.get(&value, "n").unwrap(), Value::Number(7.0));
    let method = engine.get(&value, "value").unwrap();
    assert_eq!(
        engine
            .invoke(
                method,
                Invocation {
                    receiver: Value::Number(9.0),
                    arguments: Vec::new()
                },
                &mut Bare
            )
            .unwrap(),
        Value::Number(9.0)
    );
    assert!(engine.callable(&class).is_ok());
    assert!(matches!(
        engine.invoke(
            class.clone(),
            Invocation {
                receiver: value,
                arguments: Vec::new()
            },
            &mut Bare
        ),
        Err(Failure::Throw(_))
    ));
    assert!(matches!(
        Engine::default().construct(class, Vec::new(), &mut Bare),
        Err(Failure::Fault(_))
    ));
}
