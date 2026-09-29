use super::evaluate;
use jsengine::{Engine, Rejection, Value};

#[test]
fn values() {
    for source in [
        "class C { storage = 1; get value() { return this.storage; } set value(n) { this.storage = n; } } const c = new C(); const assigned = (c.value = 7); c.value === 7 && assigned === 7 && c.storage === 7;",
        "class C { static storage = 1; static get value() { return this.storage; } static set value(n) { this.storage = n; } } C.value = 7; C.value === 7;",
        "class A { get value() { return this.n; } set value(n) { this.n = n; } } class B extends A {} const b = new B(); b.value = 7; b.value === 7 && A.prototype.n === undefined;",
        "class C { set value(n) { this.n = n; } } const c = new C(); c.value = 7; c.value === undefined && c.n === 7;",
        "class C { get value() { return 7; } } try { new C().value = 8; } catch (e) { e.constructor === TypeError; }",
        "class C { get value() { throw 7; } set value(n) { throw n; } } const c = new C(); let a = 0; try { c.value; } catch (e) { a = e; } try { c.value = 8; } catch (e) { a === 7 && e === 8; }",
        "class C { get method() { return function () { return this; }; } } const c = new C(); c.method() === c;",
        "class C { get method() { return function () { return this; }; } } const fn = new C().method; fn() === undefined;",
        "let order = ''; class C { get method() { order = order + 'g'; return function (n) { order = order + 'c'; }; } } new C().method(order = order + 'a'); order === 'gac';",
        "let order = ''; class C { set value(n) { order = order + 's'; } } const c = new C(); c[(order = order + 'k') && 'value'] = (order = order + 'v'); order === 'kvs';",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn definitions() {
    for source in [
        "class C { get value() { return 1; } set value(n) { this.n = n; } get value() { return this.n; } } const c = new C(); c.value = 7; c.value === 7;",
        "class C { set value(n) { this.n = 1; } get value() { return this.n; } set value(n) { this.n = n; } } const c = new C(); c.value = 7; c.value === 7;",
        "class C { set value(n) { throw 1; } value() {} get value() { return 7; } } const c = new C(); try { c.value = 9; } catch (e) { c.value === 7 && e.constructor === TypeError; }",
        "class C { get value() { throw 1; } value() { return 7; } } new C().value() === 7;",
        "class A { set value(n) { throw 1; } } class B extends A { get value() { return 7; } } try { new B().value = 8; } catch (e) { e.constructor === TypeError; }",
        "let count = 0; class A { set value(n) { count = count + 1; } } class B extends A { value = 7; } const b = new B(); b.value === 7 && count === 0;",
        "class C { static get value() { throw 1; } static value = 7; } C.value === 7;",
        "class C { static value = 7; static get value() { throw 1; } } C.value === 7;",
        "class C { get = 1; set() { return 2; } get getvalue() { return 3; } static get name() { return 'changed'; } } const c = new C(); c.get === 1 && c.set() === 2 && c.getvalue === 3 && C.name === 'changed';",
        "class C { static get constructor() { return 7; } get ['constructor']() { return 8; } } C.constructor === 7 && new C().constructor === 8;",
        "let order = ''; const key = function (n) { order = order + n; return 'value'; }; class C { get [key('g')]() { return this.n; } set [key('s')](n) { this.n = n; } } const c = new C(); c.value = 7; c.value === 7 && order === 'gs';",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn parent() {
    for source in [
        "class A { get value() { return this.n; } set value(n) { this.n = n; } } class B extends A { n = 1; get value() { return super.value + 1; } set value(n) { super.value = n + 1; } } const b = new B(); b.value = 7; b.value === 9 && A.prototype.n === undefined;",
        "class A { static get value() { return this.n; } static set value(n) { this.n = n; } } class B extends A { static get value() { return super.value; } static set value(n) { super.value = n; } } B.value = 7; B.value === 7 && A.n === undefined;",
        "class A { get value() { return this; } } class B extends A { read() { return super.value; } } const other = {read: B.prototype.read}; other.read() === other;",
        "class A { get value() { return this.n; } } class B extends A { n = 7; field = super.value; } new B().field === 7;",
        "class A { get value() { return 1; } } class B extends A { constructor() { super.value; } } try { new B(); } catch (e) { e.constructor === ReferenceError; }",
        "class A {} A.prototype.value = 1; let calls = 0; class B extends A { set value(n) { calls = calls + 1; } write() { super.value = 7; } } try { B.prototype.write(); } catch (e) { calls === 0 && e.constructor === TypeError; }",
        "class A {} A.prototype.value = 1; let calls = 0; class B extends A { set value(n) { calls = calls + 1; } write() { super.value = 7; } } const b = new B(); b.write(); b.value === 7 && calls === 0;",
        "class A { set value(n) { this.n = n; } } class B extends A { value = 1; write() { super.value = 7; } } const b = new B(); b.write(); b.value === 1 && b.n === 7;",
        "let count = 0; class A { get method() { count = count + 1; return function () { return this; }; } } class B extends A { method() { return super.method(); } } const b = new B(); b.method() === b && count === 1;",
        "class C { get value() { return new.target; } set value(n) { this.target = new.target; } } const c = new C(); c.value = 7; c.value === undefined && c.target === undefined;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn syntax() {
    for source in [
        "class C { get value(n) {} }",
        "class C { set value() {} }",
        "class C { set value(a, b) {} }",
        "class C { get constructor() {} }",
        "class C { set constructor(n) {} }",
        "class C { static get prototype() {} }",
        "class C { get value() { super(); } }",
        "class C { set value(n) { super(); } }",
        "class C { get value = 7; }",
        "class C { 'get' value() {} }",
        "class C { ['set'] value(n) {} }",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
}
