use jsengine::jit::Mode;
use jsengine::{Addons, Engine, Failure, Fault, Invocation, Value};

struct Bare;
impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        Err(Fault("unexpected addon".into()).into())
    }
}

fn engine(mode: Mode) -> Engine {
    let mut engine = Engine::default();
    engine.tier(mode);
    engine
}

fn numeric(value: Value) -> f64 {
    let Value::Number(value) = value else {
        panic!("expected number")
    };
    value
}

#[test]
fn differential() {
    let values = [
        0.0,
        -0.0,
        1.0,
        -7.25,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NAN,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
    ];
    for expression in [
        "x + y",
        "x - y",
        "x * y",
        "x / y",
        "-x",
        "+x",
        "x * y + 2",
        "-(x / (y - 3.25))",
        "0 / 0",
        "-0",
    ] {
        let source = format!("(x, y) => {expression};");
        let mut off = engine(Mode::Off);
        let mut eager = engine(Mode::Eager);
        let a = off.execute(&source, &mut Bare).unwrap();
        let b = eager.execute(&source, &mut Bare).unwrap();
        for x in values {
            for y in values {
                let input = vec![Value::Number(x), Value::Number(y)];
                let expected = numeric(off.call(a.clone(), input.clone(), &mut Bare).unwrap());
                let actual = numeric(eager.call(b.clone(), input, &mut Bare).unwrap());
                assert!(
                    expected.to_bits() == actual.to_bits() || expected.is_nan() && actual.is_nan(),
                    "{expression}: {x}, {y}: {expected} != {actual}"
                );
            }
        }
        let report = eager.performance();
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        assert_eq!(report.functions[0].native, 100);
        assert_eq!(report.functions[0].calls, 100);
    }
}

#[test]
fn fallback() {
    for mode in [Mode::Off, Mode::Eager, Mode::Hot(2)] {
        let mut engine = engine(mode);
        engine
            .execute("const f = (x, y) => x + y;", &mut Bare)
            .unwrap();
        assert_eq!(
            engine.execute("f(1, 2);", &mut Bare).unwrap(),
            Value::Number(3.0)
        );
        assert_eq!(
            engine.execute("f('a', 2);", &mut Bare).unwrap(),
            Value::Text("a2".into())
        );
        assert!(numeric(engine.execute("f(1);", &mut Bare).unwrap()).is_nan());
        assert_eq!(
            engine.execute("f(2, 4, 'ignored');", &mut Bare).unwrap(),
            Value::Number(6.0)
        );
        let failure = engine.execute("f({}, 2);", &mut Bare).unwrap_err();
        assert!(failure.to_string().contains("object coercion"));
        assert_eq!(
            engine
                .execute(
                    "let n = 0; const g = (x = (n += 1)) => x * 2; g(); g(); n;",
                    &mut Bare
                )
                .unwrap(),
            Value::Number(2.0)
        );
        assert_eq!(
            engine
                .execute(
                    "const outer = x => y => x + y; const c = outer(4); c(3);",
                    &mut Bare
                )
                .unwrap(),
            Value::Number(7.0)
        );
        assert_eq!(
            engine
                .execute(
                    "const effect = x => { n += x; return n; }; effect(3); effect(4);",
                    &mut Bare
                )
                .unwrap(),
            Value::Number(9.0)
        );
        assert_eq!(
            engine
                .execute(
                    "const f2 = function(x) { return x * 2; }; f2(4);",
                    &mut Bare
                )
                .unwrap(),
            Value::Number(8.0)
        );
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        if mode != Mode::Off {
            assert!(engine.performance().functions[0].guards >= 3);
        }
    }
}

#[test]
fn budgets() {
    let source = "let count = 0; const f = (x, y) => -(x * y + 2); const items = [0]; for (let n of items) { f(count, 2); count += 1; items.length += 1; }";
    let mut counts = Vec::new();
    for mode in [Mode::Off, Mode::Eager, Mode::Hot(32)] {
        let mut engine = engine(mode);
        let failure = engine.execute(source, &mut Bare).unwrap_err();
        assert!(failure.to_string().contains("execution budget exhausted"));
        counts.push(engine.execute("count;", &mut Bare).unwrap());
        assert_eq!(
            engine.execute("f(3, 2);", &mut Bare).unwrap(),
            Value::Number(-8.0)
        );
    }
    assert!(counts.windows(2).all(|pair| pair[0] == pair[1]));
    let source = "const f = x => -(-(-(-x))); let calls = 0; const recurse = x => { calls += 1; if (x < 1) { return f(2); } return recurse(x - 1); };";
    for depth in 120..132 {
        let mut results = Vec::new();
        for mode in [Mode::Off, Mode::Eager] {
            let mut engine = engine(mode);
            engine.execute(source, &mut Bare).unwrap();
            results.push(engine.execute(&format!("recurse({depth});"), &mut Bare));
        }
        assert_eq!(results[0], results[1]);
    }
}

#[test]
fn policy() {
    let mut engine = engine(Mode::Hot(3));
    let callback = engine.execute("x => x * 2;", &mut Bare).unwrap();
    for _ in 0..4 {
        engine
            .call(callback.clone(), vec![Value::Number(2.0)], &mut Bare)
            .unwrap();
    }
    let report = engine.performance();
    assert_eq!(report.functions[0].calls, 4);
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        assert_eq!(report.functions[0].native, 2);
        assert_eq!(report.functions[0].interpreted, 2);
        assert!(report.functions[0].bytes > 0);
    }
    engine.tier(Mode::Off);
    engine
        .call(callback.clone(), vec![Value::Number(3.0)], &mut Bare)
        .unwrap();
    assert_eq!(
        engine.performance().functions[0].native,
        report.functions[0].native
    );
    let mut second = Engine::default();
    assert!(second.call(callback, vec![], &mut Bare).is_err());
    assert!(second.performance().functions.is_empty());
}

#[test]
fn capacity() {
    let mut engine = engine(Mode::Eager);
    for _ in 0..260 {
        engine.execute("(x => x * 2)(3);", &mut Bare).unwrap();
    }
    let report = engine.performance();
    assert_eq!(report.functions.len(), 256);
    assert_eq!(report.omitted, 4);
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        assert_eq!(
            report
                .functions
                .iter()
                .filter(|function| function.native > 0)
                .count(),
            64
        );
        assert_eq!(
            report.functions[64].rejection.as_deref(),
            Some("compiled function budget exceeded")
        );
    }
}
