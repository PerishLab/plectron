use super::native::Bare;
use jsengine::{Addons, Engine, Failure, Fault, Value};

struct Host {
    calls: usize,
    failure: Option<Value>,
}

impl Addons for Host {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: jsengine::Invocation,
    ) -> Result<Value, Failure> {
        self.calls += 1;
        match binding {
            0 => Err(Fault("host stopped".into()).into()),
            1 => engine.call(invocation.arguments[0].clone(), Vec::new(), self),
            2 => Err(Failure::Throw(self.failure.clone().unwrap())),
            3 => engine.construct(invocation.arguments[0].clone(), Vec::new(), self),
            _ => Err(Fault("unknown binding".into()).into()),
        }
    }
}

fn prepare() -> (Engine, Host) {
    let mut engine = Engine::default();
    for (id, name) in ["stop", "invoke", "fail", "construct"]
        .into_iter()
        .enumerate()
    {
        let value = engine.native(id);
        engine.register(name, value).unwrap();
    }
    (
        engine,
        Host {
            calls: 0,
            failure: None,
        },
    )
}

#[test]
fn abort() {
    let (mut engine, mut host) = prepare();
    let failure = engine
        .execute(
            "let marker = 0; try { stop(); } catch { marker = 1; } finally { marker = 2; }",
            &mut host,
        )
        .unwrap_err();
    assert_eq!(failure, Failure::Fault(Fault("host stopped".into())));
    assert_eq!(host.calls, 1);
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    assert_eq!(
        engine
            .execute(
                "try { invoke(function () { stop(); }); } catch {}",
                &mut host
            )
            .unwrap_err(),
        failure
    );
}

#[test]
fn native() {
    let (mut engine, mut host) = prepare();
    assert_eq!(
        engine
            .execute(
                "try { invoke(function () { throw 7; }); } catch (value) { value; }",
                &mut host
            )
            .unwrap(),
        Value::Number(7.0)
    );
    let value = engine.execute("({code: 42});", &mut Bare).unwrap();
    host.failure = Some(value.clone());
    engine.register("expected", value).unwrap();
    assert_eq!(
        engine
            .execute(
                "try { fail(); } catch (value) { value === expected; }",
                &mut host
            )
            .unwrap(),
        Value::Boolean(true)
    );
    let foreign = Engine::default()
        .execute("({code: 42});", &mut Bare)
        .unwrap();
    host.failure = Some(foreign);
    let failure = engine
        .execute("try { fail(); } catch { 1; } finally { 2; }", &mut host)
        .unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("another engine"));
}

#[test]
fn budget() {
    let (mut engine, mut host) = prepare();
    let source = "let marker = 0; const items = [1]; try { for (let n of items) { invoke(function () { return 1; }); items.length = items.length + 1; } } catch { marker = 1; } finally { marker = 2; }";
    let failure = engine.execute(source, &mut host).unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("execution budget exhausted"));
    assert!(host.calls > 100);
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    let deep = "const recurse = function () { try { return recurse(); } catch { return 1; } finally { return 2; } }; recurse();";
    assert!(matches!(
        engine.execute(deep, &mut host),
        Err(Failure::Fault(_))
    ));
    let large = " ".repeat(65_537);
    assert!(matches!(
        engine.execute(&large, &mut host),
        Err(Failure::Fault(_))
    ));
    assert!(matches!(
        Engine::check(&large),
        Err(jsengine::Rejection::Fault(_))
    ));
}

#[test]
fn construction() {
    let (mut engine, mut host) = prepare();
    assert_eq!(
        engine
            .execute(
                "const C = function () { this.value = 42; }; construct(C).value;",
                &mut host
            )
            .unwrap(),
        Value::Number(42.0)
    );
    let source = "let marker = 0; const items = [1]; try { for (let n of items) { construct(C); items.length = items.length + 1; } } catch { marker = 1; } finally { marker = 2; }";
    let failure = engine.execute(source, &mut host).unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("execution budget exhausted"));
    assert!(host.calls > 100);
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    let source = "const Halt = function () { stop(); }; try { new Halt(); } catch { marker = 1; } finally { marker = 2; }";
    assert_eq!(
        engine.execute(source, &mut host).unwrap_err(),
        Failure::Fault(Fault("host stopped".into()))
    );
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    let foreign = Engine::default().object(Default::default()).unwrap();
    host.failure = Some(foreign);
    let source = "try { new (function () { fail(); })(); } catch { marker = 1; }";
    assert!(matches!(
        engine.execute(source, &mut host),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    assert_eq!(
        engine
            .execute("construct(function () { throw 7; });", &mut host)
            .unwrap_err(),
        Failure::Throw(Value::Number(7.0))
    );
}

#[test]
fn classes() {
    let (mut engine, mut host) = prepare();
    let source = "let marker = 0; class A {} class B extends A { field = stop(); constructor() { try { super(); } catch { marker = 1; } finally { marker = 2; } } } new B();";
    assert_eq!(
        engine.execute(source, &mut host).unwrap_err(),
        Failure::Fault(Fault("host stopped".into()))
    );
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    assert_eq!(engine.execute("try { class C { static field = stop(); } } catch { marker = 1; } finally { marker = 2; }", &mut host).unwrap_err(), Failure::Fault(Fault("host stopped".into())));
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    let source = "let C = class {}; const chain = [1]; for (let n of chain) { C = class extends C {}; if (chain.length < 200) { chain.length = chain.length + 1; } } try { new C(); } catch { marker = 1; } finally { marker = 2; }";
    let failure = engine.execute(source, &mut host).unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("evaluation depth exhausted"));
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    assert_eq!(
        engine.execute("new A() instanceof A;", &mut host).unwrap(),
        Value::Boolean(true)
    );
    let source = format!(
        "const C = {}class {{}}{};",
        "class extends ".repeat(100),
        " {}".repeat(100)
    );
    assert!(matches!(
        Engine::check(&source),
        Err(jsengine::Rejection::Fault(_))
    ));
}

#[test]
fn accessors() {
    let (mut engine, mut host) = prepare();
    engine.execute("let marker = 0; class C { get value() { return stop(); } set value(n) { stop(); } } const c = new C();", &mut host).unwrap();
    for source in [
        "try { c.value; } catch { marker = 1; } finally { marker = 2; }",
        "try { c.value = 7; } catch { marker = 1; } finally { marker = 2; }",
    ] {
        assert_eq!(
            engine.execute(source, &mut host).unwrap_err(),
            Failure::Fault(Fault("host stopped".into()))
        );
        assert_eq!(
            engine.execute("marker;", &mut host).unwrap(),
            Value::Number(0.0)
        );
    }
    let failure = engine.execute("class Recursive { get value() { return this.value; } } try { new Recursive().value; } catch { marker = 1; } finally { marker = 2; }", &mut host).unwrap_err();
    assert!(matches!(failure, Failure::Fault(_)));
    assert!(failure.to_string().contains("evaluation depth exhausted"));
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
    engine
        .execute(
            "class Foreign { get value() { return fail(); } }",
            &mut host,
        )
        .unwrap();
    host.failure = Some(Engine::default().object(Default::default()).unwrap());
    assert!(matches!(
        engine.execute(
            "try { new Foreign().value; } catch { marker = 1; }",
            &mut host
        ),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("marker;", &mut host).unwrap(),
        Value::Number(0.0)
    );
}
