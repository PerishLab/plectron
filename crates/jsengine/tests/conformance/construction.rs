use super::native::Bare;
use jsengine::{Engine, Failure, Value};

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn instances() {
    for source in [
        "const C = function (n) { this.n = n; }; const a = new C(7); const b = new C(8); a.n === 7 && b.n === 8 && a !== b && a instanceof C;",
        "const C = function () {}; C.prototype.value = 3; const a = new C(); C.prototype.value = 4; a.value === 4 && a.constructor === C;",
        "const C = function () {}; C.prototype.method = function () { return this; }; const a = new C(); a.method() === a;",
        "const C = function () { return this; }; const holder = {C: C}; const a = new holder.C(); a !== holder && a instanceof C;",
        "const C = function () {}; C.prototype.value = 1; const a = new C(); a.value = 2; a.value === 2 && C.prototype.value === 1;",
        "const C = function (n) { return function () { return n; }; }; new C(42)() === 42;",
        "const C = function () {}; const old = C.prototype; const a = new C(); C.prototype = {value: 9}; const b = new C(); !(a instanceof C) && b instanceof C && b.value === 9 && a.constructor === C && old !== C.prototype;",
        "const C = function () {}; C.prototype = [1, 2]; const a = new C(); C.prototype.length = 1; a.length === 1 && a[0] === 1 && a[1] === undefined;",
        "const Base = function () {}; const Child = function () {}; Child.prototype = new Base(); const a = new Child(); a instanceof Child && a instanceof Base;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn returns() {
    for value in ["undefined", "null", "false", "0", "'text'"] {
        let source = format!(
            "const C = function () {{ this.value = 7; return {value}; }}; const a = new C(); a.value === 7 && a instanceof C;"
        );
        assert_eq!(evaluate(&source), Value::Boolean(true), "{source}");
    }
    for source in [
        "const result = {}; const C = function () { return result; }; new C() === result;",
        "const result = []; const C = function () { return result; }; new C() === result;",
        "const result = function () {}; const C = function () { return result; }; new C() === result;",
        "const C = function () { try { return {}; } finally { return 7; } }; new C() instanceof C;",
        "const result = {}; const C = function () { try { return 7; } finally { return result; } }; new C() === result;",
        "const result = {}; const C = function () { throw result; }; try { new C(); } catch (error) { error === result; }",
        "const C = function () { return this; }; C() === undefined && new C() instanceof C;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn ordering() {
    for source in [
        "let order = ''; const C = function (a, b) { order = order + 'c'; this.value = a + b; }; const holder = {C: C}; const base = function () { order = order + 'b'; return holder; }; const key = function () { order = order + 'k'; return 'C'; }; const arg = function (n) { order = order + n; return n; }; const a = new (base()[key()])(arg('1'), arg('2')); order === 'bk12c' && a.value === '12';",
        "const C = function () {}; const next = {value: 7}; const a = new C(C.prototype = next); a.value === 7 && a instanceof C;",
        "const C = function () { C.prototype = {value: 9}; }; C.prototype.value = 1; const a = new C(); a.value === 1 && !(a instanceof C);",
        "let marker = 0; try { new (1)(marker = 7); } catch (error) { marker === 7 && error.constructor === TypeError; }",
        "try { new (1)((function () { throw 7; })()); } catch (error) { error === 7; }",
        "const C = function () {}; const holder = {C: C}; const a = new holder.C(holder.C = function () { return {}; }); a instanceof C;",
        "let saved = undefined; const C = function () { saved = this; this.value = 7; throw 1; }; try { new C(); } catch {} saved.value === 7 && saved instanceof C;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn matching() {
    for source in [
        "const C = function () {}; C.prototype = null; 1 instanceof C === false;",
        "const C = function () {}; C.prototype = null; try { ({}) instanceof C; } catch (error) { error.constructor === TypeError; }",
        "try { 1 instanceof {}; } catch (error) { error.constructor === TypeError; }",
        "const C = function () {}; C.prototype = C; new C() instanceof C;",
        "Error('x') instanceof Error && new TypeError('x') instanceof Error;",
        "Error.prototype.constructor === Error && TypeError.prototype.constructor === TypeError;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn roots() {
    let mut engine = Engine::default();
    let object = engine.execute("({});", &mut Bare).unwrap();
    let root = engine.prototype(&object).unwrap();
    assert_eq!(engine.prototype(&root).unwrap(), Value::Null);
    engine.set(&root, "shared", Value::Number(42.0)).unwrap();
    for value in ["null", "undefined", "1", "false", "'x'"] {
        let source = format!(
            "(function () {{ const C = function () {{}}; C.prototype = {value}; return new C(); }})();"
        );
        let instance = engine.execute(&source, &mut Bare).unwrap();
        assert_eq!(engine.prototype(&instance).unwrap(), root);
        assert_eq!(
            engine.get(&instance, "shared").unwrap(),
            Value::Number(42.0)
        );
    }
    let function = engine.execute("(function () {});", &mut Bare).unwrap();
    let prototype = engine.get(&function, "prototype").unwrap();
    assert_eq!(engine.prototype(&prototype).unwrap(), root);
    let parent = engine.prototype(&function).unwrap();
    assert_eq!(engine.prototype(&parent).unwrap(), root);
    assert_eq!(
        engine.call(parent.clone(), Vec::new(), &mut Bare).unwrap(),
        Value::Undefined
    );
    assert!(matches!(
        engine.construct(parent, Vec::new(), &mut Bare),
        Err(Failure::Throw(_))
    ));
    let native = engine.native(0);
    assert_eq!(
        engine.prototype(&native).unwrap(),
        engine.prototype(&function).unwrap()
    );
    let error = engine.execute("Error;", &mut Bare).unwrap();
    let subtype = engine.execute("TypeError;", &mut Bare).unwrap();
    assert_eq!(
        engine.prototype(&error).unwrap(),
        engine.prototype(&function).unwrap()
    );
    assert_eq!(engine.prototype(&subtype).unwrap(), error);
    let prototype = engine.get(&error, "prototype").unwrap();
    assert_eq!(engine.prototype(&prototype).unwrap(), root);
}

#[test]
fn capabilities() {
    let mut engine = Engine::default();
    let native = engine.native(0);
    let prototype = engine.object(Default::default()).unwrap();
    engine.set(&native, "prototype", prototype).unwrap();
    engine.register("native", native).unwrap();
    assert_eq!(
        engine
            .execute(
                "try { new native(); } catch (error) { error.constructor === TypeError; }",
                &mut Bare
            )
            .unwrap(),
        Value::Boolean(true)
    );
    let source =
        "const C = function () {}; C.prototype = native.prototype; new C() instanceof native;";
    assert_eq!(
        engine.execute(source, &mut Bare).unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn ownership() {
    let mut engine = Engine::default();
    let function = engine
        .execute("let marker = 0; (function () { marker = 1; });", &mut Bare)
        .unwrap();
    let foreign = Engine::default().object(Default::default()).unwrap();
    for (callee, arguments) in [
        (foreign.clone(), Vec::new()),
        (function, vec![foreign.clone()]),
    ] {
        let failure = engine.construct(callee, arguments, &mut Bare).unwrap_err();
        assert!(matches!(failure, Failure::Fault(_)));
        assert!(failure.to_string().contains("another engine"));
    }
    assert_eq!(
        engine.execute("marker;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
    assert!(engine.prototype(&foreign).is_err());
    let value = engine.execute("new Error();", &mut Bare).unwrap();
    assert!(Engine::default().prototype(&value).is_err());
}
