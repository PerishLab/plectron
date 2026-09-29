use jsengine::{Addons, Engine, Failure, Fault, Invocation, Value, jit::Mode};

struct Host;
impl Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        Err(Fault("unexpected addon".into()).into())
    }
}

fn check(source: &str, assertion: &str) {
    for mode in [Mode::Off, Mode::Eager, Mode::Hot(2)] {
        let mut engine = Engine::default();
        engine.tier(mode);
        engine
            .execute(source, &mut Host)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        engine.checkpoint(&mut Host).unwrap();
        assert_eq!(
            engine.execute(assertion, &mut Host).unwrap(),
            Value::Boolean(true)
        );
        assert!(engine.jobs().rejected.is_empty());
    }
}

#[test]
fn scopes() {
    check(
        "const make = seed => { let value = seed; return (step = 1) => { value += step; return value; }; }; const a = make(0); const b = make(10); const first = a(); const second = b(); const third = a(2); const fourth = b(3);",
        "first === 1 && second === 11 && third === 3 && fourth === 14;",
    );
    check(
        "const collect = function(n, values = []) { values[values.length] = n; if (n === 0) { return values; } return collect(n - 1, values); }; const a = collect(4); const b = collect(2);",
        "a !== b && a.length === 5 && b.length === 3 && a[0] === 4 && b[0] === 2;",
    );
}

#[test]
fn homes() {
    check(
        "const make = seed => { const Base = class { value() { return seed; } }; return class extends Base { value() { return super.value() + 1; } }; }; const A = make(3); const B = make(10); const a = new A(); const b = new B();",
        "a.value() === 4 && b.value() === 11 && a.value() === 4;",
    );
}

#[test]
fn continuations() {
    check(
        "let completed = 0; let a = null; let b = null; let failed = null; const task = async (value, fail = false, state = []) => { state[0] = value; await 0; try { if (fail) { throw value; } await 1; return () => state[0]; } finally { await 0; completed += 1; } }; task(3).then(f => { a = f; }); task(7).then(f => { b = f; }); task(11, true).catch(value => { failed = value; });",
        "a() === 3 && b() === 7 && failed === 11 && completed === 3;",
    );
}
