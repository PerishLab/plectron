use jsengine::{Addons, Engine, Failure, Fault, Value};
use std::collections::BTreeMap;

pub struct Bare;

#[test]
fn derivation() {
    let mut engine = Engine::default();
    let prototype = engine
        .object([("answer".into(), Value::Number(42.0))].into())
        .unwrap();
    let value = engine.derive(&prototype).unwrap();
    assert_eq!(engine.prototype(&value).unwrap(), prototype);
    assert!(engine.properties(&value).unwrap().is_empty());
    assert_eq!(engine.get(&value, "answer").unwrap(), Value::Number(42.0));
    let null = engine.derive(&Value::Null).unwrap();
    assert_eq!(engine.prototype(&null).unwrap(), Value::Null);
    assert!(engine.derive(&Value::Number(1.0)).is_err());
    assert!(Engine::default().derive(&prototype).is_err());
}

#[test]
fn invocation() {
    let mut engine = Engine::default();
    assert_eq!(
        engine
            .execute(
                "const f = function (x) { return this.base + x; }; f.call({base: 40}, 2);",
                &mut Bare
            )
            .unwrap(),
        Value::Number(42.0)
    );
    assert_eq!(engine.execute("let caught = false; try { f.call.call({}, null); } catch (error) { caught = error instanceof TypeError; } caught;", &mut Bare).unwrap(), Value::Boolean(true));
    assert_eq!(
        engine
            .execute("const arrow = () => this; arrow.call({});", &mut Bare)
            .unwrap(),
        Value::Undefined
    );
}

impl Addons for Bare {
    fn call(
        &mut self,
        _: &mut Engine,
        _: usize,
        _: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        Err(Fault("no addons installed".into()).into())
    }
}

#[derive(Default)]
struct Echo {
    calls: usize,
}

impl Addons for Echo {
    fn call(
        &mut self,
        _: &mut Engine,
        binding: usize,
        invocation: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        if binding != 7 {
            return Err(Fault("unknown addon operation".into()).into());
        }
        self.calls += 1;
        Ok(invocation
            .arguments
            .first()
            .cloned()
            .unwrap_or(Value::Undefined))
    }
}

#[test]
fn bare() {
    let mut engine = Engine::default();
    for source in ["platform;", "console;", "surface;"] {
        let failure = engine.execute(source, &mut Bare).unwrap_err();
        assert!(engine.diagnostic(&failure).0.contains("unknown binding"));
    }
    assert_eq!(
        engine.execute("2 + 3;", &mut Bare).unwrap(),
        Value::Number(5.0)
    );
}

#[test]
fn registration() {
    let mut engine = Engine::default();
    assert!(engine.binding("tools").is_err());
    let native = engine.native(7);
    let namespace = engine
        .object(BTreeMap::from([("echo".into(), native)]))
        .unwrap();
    engine.register("tools", namespace.clone()).unwrap();
    assert_eq!(engine.binding("tools").unwrap(), namespace);
    let mut addons = Echo::default();
    let result = engine.execute("tools.echo(42);", &mut addons).unwrap();
    assert_eq!(result, Value::Number(42.0));
    assert_eq!(addons.calls, 1);
}

#[test]
fn callback() {
    let mut engine = Engine::default();
    let callback = engine
        .execute(
            "let n = 0; (function (step) { n = n + step; return n; });",
            &mut Bare,
        )
        .unwrap();
    assert_eq!(
        engine
            .call(callback.clone(), vec![Value::Number(2.0)], &mut Bare)
            .unwrap(),
        Value::Number(2.0)
    );
    assert_eq!(
        engine
            .call(callback, vec![Value::Number(3.0)], &mut Bare)
            .unwrap(),
        Value::Number(5.0)
    );
}

#[test]
fn ownership() {
    let mut source = Engine::default();
    let mut target = Engine::default();
    let object = source.execute("({x: 1});", &mut Bare).unwrap();
    let callback = source
        .execute("(function () { return 1; });", &mut Bare)
        .unwrap();
    let native = source.native(7);
    for value in [object, callback, native] {
        let error = target.register("foreign", value.clone()).unwrap_err();
        assert!(error.0.contains("another engine"));
        assert!(
            target
                .object(BTreeMap::from([("foreign".into(), value.clone())]))
                .is_err()
        );
        assert!(target.call(value, Vec::new(), &mut Bare).is_err());
    }
    let number = source.execute("42;", &mut Bare).unwrap();
    target.register("copied", number).unwrap();
    assert_eq!(
        target.execute("copied;", &mut Bare).unwrap(),
        Value::Number(42.0)
    );
}

#[test]
fn handles() {
    let mut engine = Engine::default();
    let value = engine.execute("(function () {});", &mut Bare).unwrap();
    engine
        .set(&value, "tag", Value::Text("function".into()))
        .unwrap();
    engine.register("callback", value).unwrap();
    assert_eq!(
        engine.execute("callback.tag;", &mut Bare).unwrap(),
        Value::Text("function".into())
    );
    let callback = engine.execute("callback;", &mut Bare).unwrap();
    engine.callable(&callback).unwrap();
    assert!(engine.register("undefined", Value::Null).is_err());
}

struct Foreign(Value);

impl Addons for Foreign {
    fn call(
        &mut self,
        _: &mut Engine,
        _: usize,
        _: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        Ok(self.0.clone())
    }
}

#[test]
fn returns() {
    let mut source = Engine::default();
    let object = source.object(BTreeMap::new()).unwrap();
    let mut target = Engine::default();
    let native = target.native(0);
    target.register("foreign", native).unwrap();
    assert!(
        target
            .execute("foreign();", &mut Foreign(object))
            .unwrap_err()
            .to_string()
            .contains("another engine")
    );
}

#[test]
fn parsing() {
    let mut engine = Engine::default();
    let native = engine.native(7);
    engine.register("effect", native).unwrap();
    let mut addons = Echo::default();
    assert!(
        engine
            .execute("effect(); while (true) {}", &mut addons)
            .is_err()
    );
    assert!(
        engine
            .execute(
                "effect(); if (false) { let x = 1; let x = 2; }",
                &mut addons
            )
            .is_err()
    );
    assert_eq!(addons.calls, 0);
}

#[test]
fn failures() {
    let mut engine = Engine::default();
    let native = engine.native(99);
    engine.register("fail", native).unwrap();
    let error = engine.execute("fail();", &mut Echo::default()).unwrap_err();
    assert_eq!(
        error,
        Failure::Fault(Fault("unknown addon operation".into()))
    );
}
