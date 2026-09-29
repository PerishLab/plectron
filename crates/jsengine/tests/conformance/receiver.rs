use super::native::Bare;
use jsengine::{Addons, Engine, Failure, Fault, Invocation, Value};
use std::collections::BTreeMap;

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn methods() {
    for source in [
        "const object = {method: function () { return this; }}; object.method() === object;",
        "const object = {method: function () { return this; }}; (object['method'])() === object;",
        "const method = function () { return this; }; const a = {method: method}; const b = {method: method}; a.method() === a && b.method() === b;",
        "const object = {method: function () { return this; }}; const detached = object.method; detached() === undefined;",
        "const object = {method: function () { return this; }}; (object.method || undefined)() === undefined;",
        "const object = {method: function () { return this; }}; let detached = undefined; (detached = object.method)() === undefined;",
        "const plain = function () { return this; }; const object = {method: function () { return plain(); }}; object.method() === undefined;",
        "const object = {method: function () { return function () { return this; }; }}; object.method()() === undefined;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn scopes() {
    assert_eq!(
        evaluate(
            "const object = {method: function () { for (let n of [1]) { try { throw n; } catch { return this; } } }}; object.method() === object;"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "const object = {method: function () { const saved = this; return function () { return saved; }; }}; object.method()() === object;"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "const object = {method: function () { try { (function () { throw 1; })(); } catch {} return this; }}; object.method() === object;"
        ),
        Value::Boolean(true)
    );
    assert_eq!(evaluate("this;"), Value::Undefined);
    assert!(Engine::check("this = 1;").is_err());
    assert!(Engine::check("function (this) {};").is_err());
}

#[test]
fn references() {
    assert_eq!(
        evaluate(
            "let n = 0; const object = {method: function () { return this; }}; const base = function () { n = n + 1; return object; }; const key = function () { n = n + 1; return 'method'; }; base()[key()]() === object && n === 2;"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "const old = {method: function () { return this; }}; let object = old; const other = {}; object.method(object = other) === old;"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "const object = {method: function () { return 1; }}; object.method(object.method = function () { return 2; });"
        ),
        Value::Number(1.0)
    );
    assert_eq!(
        evaluate(
            "let n = 0; const key = function () { n = n + 1; return 'x'; }; try { null[key()] = (n = 2); } catch {} n;"
        ),
        Value::Number(2.0)
    );
    assert_eq!(
        evaluate(
            "let n = 0; const key = function () { throw 7; }; try { null[key()] = (n = 2); } catch (error) { n = error; } n;"
        ),
        Value::Number(7.0)
    );
}

struct Host;

impl Addons for Host {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: Invocation,
    ) -> Result<Value, Failure> {
        match binding {
            0 => Ok(invocation.receiver),
            1 => {
                engine.register("late", Value::Number(42.0))?;
                Ok(Value::Number(1.0))
            }
            _ => Err(Fault("unknown binding".into()).into()),
        }
    }
}

#[test]
fn native() {
    let mut engine = Engine::default();
    let method = engine.native(0);
    engine.register("method", method.clone()).unwrap();
    let object = engine
        .object(BTreeMap::from([("method".into(), method.clone())]))
        .unwrap();
    engine.register("object", object.clone()).unwrap();
    assert_eq!(
        engine.execute("object.method();", &mut Host).unwrap(),
        object
    );
    assert_eq!(
        engine.execute("method();", &mut Host).unwrap(),
        Value::Undefined
    );
    let callback = engine
        .execute("(function () { return this; });", &mut Bare)
        .unwrap();
    for receiver in [
        Value::Undefined,
        Value::Null,
        Value::Boolean(false),
        Value::Number(7.0),
        Value::Text("text".into()),
        object,
    ] {
        let invocation = Invocation {
            receiver: receiver.clone(),
            arguments: Vec::new(),
        };
        assert_eq!(
            engine
                .invoke(callback.clone(), invocation.clone(), &mut Host)
                .unwrap(),
            receiver
        );
        assert_eq!(
            engine
                .invoke(method.clone(), invocation, &mut Host)
                .unwrap(),
            receiver
        );
    }
    assert_eq!(
        engine.call(callback, Vec::new(), &mut Host).unwrap(),
        Value::Undefined
    );
}

#[test]
fn resolution() {
    let mut engine = Engine::default();
    let native = engine.native(1);
    engine.register("register", native).unwrap();
    assert_eq!(engine.execute("try { late = register(); } catch (error) { error.constructor === ReferenceError; }", &mut Host).unwrap(), Value::Boolean(true));
    assert_eq!(
        engine.execute("late;", &mut Host).unwrap(),
        Value::Number(42.0)
    );
}

#[test]
fn ownership() {
    let mut engine = Engine::default();
    let callback = engine
        .execute("(function () { return this; });", &mut Bare)
        .unwrap();
    let foreign = Engine::default().object(BTreeMap::new()).unwrap();
    let failure = engine
        .invoke(
            callback,
            Invocation {
                receiver: foreign,
                arguments: Vec::new(),
            },
            &mut Host,
        )
        .unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("another engine"));
}

#[test]
fn precedence() {
    assert_eq!(
        evaluate("try { null[{}]; } catch (error) { error.constructor === TypeError; }"),
        Value::Boolean(true)
    );
    let mut engine = Engine::default();
    let failure = engine.execute("({})[{}];", &mut Bare).unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("key coercion is unsupported"));
}

#[test]
fn deferred() {
    assert_eq!(
        evaluate("try { null.x = (function () { throw 'rhs'; })(); } catch (error) { error; }"),
        Value::Text("rhs".into())
    );
    let mut engine = Engine::default();
    engine.execute("let marker = 0;", &mut Bare).unwrap();
    let failure = engine
        .execute("({})[{}] = (marker = 1);", &mut Bare)
        .unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("key coercion is unsupported"));
    assert_eq!(
        engine.execute("marker;", &mut Bare).unwrap(),
        Value::Number(1.0)
    );
}
