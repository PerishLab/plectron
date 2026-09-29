use super::evaluate;
use crate::native::Bare;
use jsengine::{Engine, Failure, Rejection, Value};

#[test]
fn construction() {
    for source in [
        "class A { constructor(n) { this.n = n; this.target = new.target; } } class B extends A {} class C extends B {} const c = new C(7); c.n === 7 && c.target === C && c instanceof A && c instanceof B && c instanceof C;",
        "const A = function (n) { this.n = n; this.target = new.target; }; class B extends A { constructor(n) { super(n + 1); this.m = 2; } } const b = new B(6); b.n === 7 && b.m === 2 && b.target === B && b instanceof A;",
        "class E extends TypeError { detail = 7; } const e = new E('bad'); e instanceof E && e instanceof TypeError && e instanceof Error && e.message === 'bad' && e.name === 'TypeError' && e.detail === 7;",
        "class A {} class B extends A { constructor() { const target = new.target; super(); this.target = target; } } new B().target === B;",
        "const f = function () { return new.target; }; f() === undefined && new f() === f;",
        "class C { field = new.target; static field = new.target; method() { return new.target; } constructor() { this.nested = (function () { return new.target; })(); } } const c = new C(); c.field === undefined && C.field === undefined && c.method() === undefined && c.nested === undefined;",
        "class A {} class B extends A {} B.length === 0 && B.name === 'B' && new B().constructor === B;",
        "class A {} class B extends A { constructor() { super(); } } try { B(); } catch (e) { e.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn returns() {
    for source in [
        "class A {} class B extends A { constructor() { return {}; } } const b = new B(); !(b instanceof B);",
        "class A {} class B extends A { constructor() { return 1; } } try { new B(); } catch (e) { e.constructor === TypeError; }",
        "class A {} class B extends A { constructor() { super(); return null; } } try { new B(); } catch (e) { e.constructor === TypeError; }",
        "class A {} class B extends A { constructor() { return undefined; } } try { new B(); } catch (e) { e.constructor === ReferenceError; }",
        "class A {} class B extends A { constructor() {} } try { new B(); } catch (e) { e.constructor === ReferenceError; }",
        "class A {} class B extends A { constructor() { super(); return undefined; } } new B() instanceof B;",
        "class A {} class B extends A { constructor() { try { return 1; } finally { return {}; } } } !(new B() instanceof B);",
        "const result = {}; class A { discarded = 1; constructor() { return result; } } class B extends A { kept = 2; } new B() === result && result.kept === 2 && result.discarded === undefined;",
        "let fields = 0; class A {} class B extends A { value = (fields = fields + 1); constructor() { return {}; } } new B(); fields === 0;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn initialization() {
    for source in [
        "class A {} class B extends A { constructor() { this.x = 1; super(); } } try { new B(); } catch (e) { e.constructor === ReferenceError; }",
        "let calls = 0; let fields = 0; class A { constructor() { calls = calls + 1; } } class B extends A { value = (fields = fields + 1); constructor() { super(); try { super(); } catch (e) { this.error = e.constructor; } } } const b = new B(); calls === 2 && fields === 1 && b.error === ReferenceError;",
        "let calls = 0; class A { constructor() { calls = calls + 1; if (calls === 1) { throw 7; } } } class B extends A { constructor() { try { super(); } catch {} super(); } } new B() instanceof B && calls === 2;",
        "const fail = function () { throw 7; }; class A {} class B extends A { first = 1; failed = fail(); last = 3; constructor() { try { super(); } catch (e) { this.error = e; } } } const b = new B(); b.first === 1 && b.failed === undefined && b.last === undefined && b.error === 7;",
        "class A {} class B extends A { constructor() { try { this; } catch (e) { if (e.constructor !== ReferenceError) { throw e; } } super(); } } new B() instanceof B;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn heritage() {
    for source in [
        "try { class C extends C {} } catch (e) { e.constructor === ReferenceError; }",
        "try { class C extends Missing {} } catch (e) { e.constructor === ReferenceError; }",
        "try { class C extends {} {} } catch (e) { e.constructor === TypeError; }",
        "class A { static method() {} } try { class C extends A.method {} } catch (e) { e.constructor === TypeError; }",
        "const A = function () {}; A.prototype = 7; try { class C extends A {} } catch (e) { e.constructor === TypeError; }",
        "class C extends null {} try { new C(); } catch (e) { e.constructor === TypeError; }",
        "class C extends null { constructor() { return {}; } } !(new C() instanceof C);",
        "const A = function () {}; A.prototype = null; class B extends A {} new B() instanceof B;",
        "let calls = 0; const parent = function () { calls = calls + 1; return class {}; }; class C extends parent() {} new C(); new C(); calls === 1;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    let mut engine = Engine::default();
    let class = engine
        .execute("class C extends null {} C;", &mut Bare)
        .unwrap();
    let prototype = engine.get(&class, "prototype").unwrap();
    assert_eq!(engine.prototype(&prototype).unwrap(), Value::Null);
    let parent = engine.prototype(&class).unwrap();
    assert!(engine.callable(&parent).is_ok());
    assert!(matches!(
        engine.construct(parent, Vec::new(), &mut Bare),
        Err(Failure::Throw(_))
    ));
}

#[test]
fn syntax() {
    for source in [
        "new.target;",
        "class C { [new.target]() {} }",
        "super.x;",
        "const f = function () { return super.x; };",
        "class C { constructor() { super(); } }",
        "class A {} class B extends A { method() { super(); } }",
        "class A {} class B extends A { constructor() { const f = function () { super(); }; } }",
        "class A {} class B extends A { constructor() { new super()(); } }",
        "class A {} class B extends A { field = super(); }",
        "class A {} class B extends A { static field = super(); }",
        "class C { field = arguments; }",
        "class C { static field = arguments; }",
        "class C { constructor = 1; }",
        "class C { static constructor; }",
        "class C { static prototype = 1; }",
        "class C { field = 1 }",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
    for source in [
        "class C { field = function () { return arguments; }; }",
        "class C { field = {arguments: 1}; }",
        "class C { field = new.target; }",
    ] {
        assert!(Engine::check(source).is_ok(), "{source}");
    }
}
