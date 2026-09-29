use super::evaluate;
use crate::native::Bare;
use jsengine::{Engine, Failure, Value};

#[test]
fn ordering() {
    for source in [
        "let order = ''; const mark = function (s) { order = order + s; return s; }; class A { [mark('a')] = mark('i'); static [mark('b')] = mark('s'); [mark('c')] = mark('j'); static [mark('d')] = mark('t'); constructor() { mark('A'); } } class B extends A { field = mark('k'); constructor() { mark('B'); super(); mark('C'); } } new B(); order === 'abcdstBijAkC';",
        "class C { static x = this.later(); static later() { return 7; } } C.x === 7;",
        "class C { static instance = new this(); later = 7; } C.instance.later === 7;",
        "class C { static self = C; self = C; } C.self === C && new C().self === C;",
        "let saved = undefined; const fail = function () { throw 7; }; try { class C { static first = (saved = C); static failed = fail(); static last = 3; } } catch {} saved.first === saved && saved.failed === undefined && saved.last === undefined;",
        "let marker = 0; try { const C = class { static x = C; }; } catch (e) { marker = e.constructor === ReferenceError; } marker;",
        "let marker = 0; try { class C { static ['prototype'] = 7; [(marker = 1)] = 2; } } catch (e) { marker === 1 && e.constructor === TypeError; }",
        "let n = 3; class C { field = n; constructor(n) { this.parameter = n; } } const c = new C(7); c.field === 3 && c.parameter === 7;",
        "let n = 0; class C { field = (n = 1); constructor() { this.order = n; } } new C().order === 1;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn values() {
    for source in [
        "class C { value = {}; list = []; } const a = new C(); const b = new C(); a.value !== b.value && a.list !== b.list;",
        "class C { value = 1; value = this.value + 1; empty; } const c = new C(); c.value === 2 && c.empty === undefined;",
        "class C { method = function () { return this; }; type = class {}; } const c = new C(); c.method() === c && c.method.name === 'method' && c.type.name === 'type';",
        "class C { static name = 'changed'; static length = 7; } C.name === 'changed' && C.length === 7;",
        "class C { static = 1; static static = 2; ['constructor'] = 3; __proto__ = 4; } const c = new C(); c.static === 1 && C.static === 2 && c.constructor === 3 && c.__proto__ === 4;",
        "class C { static method = 7; static method() { return 1; } } C.method === 7;",
        "class A { constructor() { return []; } } class B extends A { [3] = 7; } const b = new B(); b.length === 4 && b[3] === 7;",
        "const make = function (n) { return class { value = n; }; }; const A = make(1); const B = make(2); new A().value === 1 && new B().value === 2;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn properties() {
    let mut engine = Engine::default();
    let value = engine
        .execute("class C { value; } new C();", &mut Bare)
        .unwrap();
    let property = engine.descriptor(&value, "value").unwrap().unwrap();
    assert_eq!(property.value().unwrap(), Value::Undefined);
    assert!(property.writable() == Some(true) && property.enumerable && property.configurable);
    let class = engine.execute("class A {} A;", &mut Bare).unwrap();
    let prototype = engine.get(&class, "prototype").unwrap();
    engine.set(&prototype, "value", Value::Number(1.0)).unwrap();
    engine.freeze(&prototype).unwrap();
    let value = engine
        .execute("class B extends A { value = 7; } new B();", &mut Bare)
        .unwrap();
    assert_eq!(engine.get(&value, "value").unwrap(), Value::Number(7.0));
    assert_eq!(engine.get(&prototype, "value").unwrap(), Value::Number(1.0));
    let result = engine.object(Default::default()).unwrap();
    engine.freeze(&result).unwrap();
    engine.register("result", result).unwrap();
    let failure = engine.execute("class P { constructor() { return result; } } class Q extends P { field = 1; } new Q();", &mut Bare).unwrap_err();
    assert!(matches!(failure, Failure::Throw(_)));
    assert!(engine.diagnostic(&failure).0.contains("TypeError"));
}
