use super::super::native::Bare;
use jsengine::{Addons, Engine, Failure, Fault, Invocation, Source, Value};

fn check(failure: &Failure, identity: u64, expected: &str) {
    let site = failure.site().expect("source location");
    assert_eq!(site.source.identity(), identity);
    assert_eq!(&site.source.text()[site.span.clone()], expected);
}

#[test]
fn parameters() {
    for text in [
        "(function (value = missing) { return value; });",
        "((value = missing) => value);",
    ] {
        let mut engine = Engine::default();
        let source = Source::new(text);
        let identity = source.identity();
        let callback = engine.source(&source, &mut Bare).unwrap();
        let root = engine.retain(&callback).unwrap();
        drop(source);
        engine
            .source(&Source::new("const unrelated = '𐐀';"), &mut Bare)
            .unwrap();
        engine.collect(&mut Bare).unwrap();
        let failure = engine
            .call(engine.held(root).unwrap(), Vec::new(), &mut Bare)
            .unwrap_err();
        check(&failure, identity, "missing");
        assert!(failure.thrown().is_some());
        assert_eq!(
            engine
                .call(
                    engine.held(root).unwrap(),
                    vec![Value::Number(7.0)],
                    &mut Bare
                )
                .unwrap(),
            Value::Number(7.0)
        );
        engine.release(root).unwrap();
    }
}

#[test]
fn caller() {
    let mut engine = Engine::default();
    let definition = Source::new("const fail = function (value = missing) { return value; };");
    engine.source(&definition, &mut Bare).unwrap();
    let caller = Source::new("fail();");
    let failure = engine.source(&caller, &mut Bare).unwrap_err();
    check(&failure, definition.identity(), "missing");
    let caller = Source::new("try { fail(); } catch {} throw 'caller';");
    let failure = engine.source(&caller, &mut Bare).unwrap_err();
    check(&failure, caller.identity(), "throw 'caller';");
}

#[test]
fn asynchronous() {
    let mut engine = Engine::default();
    let source = Source::new(
        "const fail = async function (value = missing) { return value; }; fail().then();",
    );
    engine.source(&source, &mut Bare).unwrap();
    engine.checkpoint(&mut Bare).unwrap();
    let failures = engine.rejections();
    assert_eq!(failures.len(), 1);
    check(&failures[0], source.identity(), "missing");
}

#[test]
fn fields() {
    for text in [
        "(class { value = missing; });",
        "(class extends (class {}) { value = missing; });",
        "(class { value = this.absent(); });",
    ] {
        let mut engine = Engine::default();
        let source = Source::new(text);
        let identity = source.identity();
        let constructor = engine.source(&source, &mut Bare).unwrap();
        let root = engine.retain(&constructor).unwrap();
        drop(source);
        engine.execute("const unrelated = 1;", &mut Bare).unwrap();
        engine.collect(&mut Bare).unwrap();
        let failure = engine
            .construct(engine.held(root).unwrap(), Vec::new(), &mut Bare)
            .unwrap_err();
        check(
            &failure,
            identity,
            if text.contains("missing") {
                "missing"
            } else {
                "this.absent()"
            },
        );
        engine.release(root).unwrap();
    }
}

#[test]
fn immediate() {
    let mut engine = Engine::default();
    let source = Source::new("const text = '𐐀'; class Example { static value = missing; }");
    let failure = engine.source(&source, &mut Bare).unwrap_err();
    check(&failure, source.identity(), "missing");
    assert_eq!(
        failure.site().unwrap().span.start,
        source.text().find("missing").unwrap()
    );
}

#[test]
fn naming() {
    let mut engine = Engine::default();
    let source = Source::new(
        "const defaults = function (callback = function () {}, arrow = () => 1, model = class {}) { return callback.name === 'callback' && arrow.name === 'arrow' && model.name === 'model'; }; class Example { method = function () {}; arrow = () => 1; model = class {}; } const item = new Example(); defaults() && item.method.name === 'method' && item.arrow.name === 'arrow' && item.model.name === 'model';",
    );
    assert_eq!(
        engine.source(&source, &mut Bare).unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn nested() {
    let mut engine = Engine::default();
    let definition = Source::new("const nested = function () { throw 'nested'; };");
    engine.source(&definition, &mut Bare).unwrap();
    for text in [
        "(function (value = nested()) {})();",
        "new (class { value = nested(); })();",
    ] {
        let source = Source::new(text);
        let failure = engine.source(&source, &mut Bare).unwrap_err();
        check(&failure, definition.identity(), "throw 'nested';");
    }
}

struct Halt;
impl Addons for Halt {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        Err(Fault("host stopped".into()).into())
    }
}

#[test]
fn checkpoint() {
    for (text, expected) in [
        (
            "Promise.resolve().then(function (value = stop()) {});",
            "stop()",
        ),
        (
            "Promise.resolve(1).then(function () { stop(); });",
            "stop();",
        ),
        (
            "Promise.resolve(1).finally(function () { stop(); });",
            "stop();",
        ),
        ("(async function () { await 1; stop(); })();", "stop();"),
        (
            "Promise.resolve({then: function () { stop(); }});",
            "stop();",
        ),
    ] {
        let mut engine = Engine::default();
        let stop = engine.native(0);
        engine.register("stop", stop).unwrap();
        let source = Source::new(text);
        let identity = source.identity();
        engine.source(&source, &mut Halt).unwrap();
        drop(source);
        engine.collect(&mut Halt).unwrap();
        let failure = engine.checkpoint(&mut Halt).unwrap_err();
        check(&failure, identity, expected);
        assert_eq!(
            failure.cause(),
            &Failure::Fault(Fault("host stopped".into()))
        );
        assert!(engine.rejections().is_empty());
        assert_eq!(engine.diagnostic(&failure), Fault("host stopped".into()));
        engine
            .execute("Promise.resolve(1).then(function () {});", &mut Bare)
            .unwrap();
        assert!(engine.checkpoint(&mut Bare).unwrap() > 0);
        assert_eq!(engine.checkpoint(&mut Bare).unwrap(), 0);
    }
}

#[test]
fn budget() {
    let mut engine = Engine::default();
    let source =
        Source::new("const repeat = function () { repeat(); }; Promise.resolve(1).then(repeat);");
    engine.source(&source, &mut Bare).unwrap();
    let failure = engine.checkpoint(&mut Bare).unwrap_err();
    check(&failure, source.identity(), "repeat();");
    assert!(matches!(failure.cause(), Failure::Fault(_)));
    assert!(engine.rejections().is_empty());
}
