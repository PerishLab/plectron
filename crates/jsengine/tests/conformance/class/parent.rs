use super::evaluate;
use crate::native::Bare;
use jsengine::{Engine, Invocation, Value};

#[test]
fn methods() {
    for source in [
        "class A { method() { return this.value; } static method() { return this.value; } } class B extends A { value = 7; static value = 8; method() { return super.method() + 1; } static method() { return super.method() + 1; } } new B().method() === 8 && B.method() === 9;",
        "class A { method() { return this; } } class B extends A { method() { return super.method(); } } const other = {method: B.prototype.method}; other.method() === other;",
        "class A { method() { return this; } } class B extends A { method() { const detached = super.method; return detached(); } } new B().method() === undefined;",
        "class A { method() { return 1; } } class B extends A { method() { return super.method(); } } A.prototype.method = function () { return 7; }; new B().method() === 7;",
        "const A = function () {}; A.prototype.method = function () { return 1; }; class B extends A { method() { return super.method(); } } A.prototype = {method: function () { return 2; }}; new B().method() === 1;",
        "class A { method() { return this.value; } static method() { return this.value; } } class B extends A { value = 7; field = super.method(); static value = 8; static field = super.method(); } new B().field === 7 && B.field === 8;",
        "class A { constructor() { this.value = 7; } method() { return this.value; } } class B extends A { constructor() { super(); this.field = super.method(); } } new B().field === 7;",
        "class A { method() { return this; } } class B extends A { method() { class C { [super.method() === this && 'field'] = 7; } return new C().field; } } new B().method() === 7;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn writes() {
    for source in [
        "class A {} A.prototype.value = 1; class B extends A { write() { super.value = 7; return this.value; } } const b = new B(); b.write() === 7 && A.prototype.value === 1 && B.prototype.value === 1;",
        "class A { static value = 1; } class B extends A { static write() { super.value = 7; } } B.write(); B.value === 7 && A.value === 1;",
        "class A {} class B extends A { write() { super.value = 7; } } const other = {write: B.prototype.write}; other.write(); other.value === 7 && B.prototype.value === undefined;",
        "let order = ''; class A {} class B extends A { write() { super[(order = order + 'k')] = (order = order + 'v'); } } const b = new B(); b.write(); order === 'kv' && b.k === 'kv';",
        "let marker = 0; class A {} class B extends A { constructor() { super[(marker = 1)] = (marker = 2); } } try { new B(); } catch (e) { marker === 0 && e.constructor === ReferenceError; }",
        "let order = ''; class C extends null { write() { super[(order = order + 'k')] = (order = order + 'v'); } } const other = {write: C.prototype.write}; try { other.write(); } catch (e) { order === 'kv' && e.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn receivers() {
    let mut engine = Engine::default();
    let method = engine.execute("class A { read() { return this; } } class B extends A { read() { return super.read(); } write() { super.value = 7; } } B.prototype.read;", &mut Bare).unwrap();
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
    let base = engine.execute("A.prototype;", &mut Bare).unwrap();
    engine.set(&base, "value", Value::Number(1.0)).unwrap();
    engine.freeze(&base).unwrap();
    assert_eq!(
        engine
            .execute(
                "try { new B().write(); } catch (e) { e.constructor === TypeError; }",
                &mut Bare
            )
            .unwrap(),
        Value::Boolean(true)
    );
    let method = engine
        .execute(
            "class P {} class Q extends P { write() { super.value = 7; } } Q.prototype.write;",
            &mut Bare,
        )
        .unwrap();
    let receiver = engine.object(Default::default()).unwrap();
    engine.freeze(&receiver).unwrap();
    let failure = engine
        .invoke(
            method,
            Invocation {
                receiver,
                arguments: Vec::new(),
            },
            &mut Bare,
        )
        .unwrap_err();
    assert!(engine.diagnostic(&failure).0.contains("TypeError"));
}
