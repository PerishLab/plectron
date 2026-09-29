use jsengine::{Addons, Engine, Failure, Fault, Invocation, Rejection, Value};

struct Host;

impl Addons for Host {
    fn call(
        &mut self,
        engine: &mut Engine,
        binding: usize,
        invocation: Invocation,
    ) -> Result<Value, Failure> {
        match binding {
            0 => Err(Fault("host stopped".into()).into()),
            1 => engine.call(invocation.arguments[0].clone(), vec![], self),
            _ => panic!("unexpected native call"),
        }
    }
}

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Host)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn expressions() {
    for source in [
        "const f = x => x + 1; f(2) === 3;",
        "const f = (x, y) => x + y; f(2, 3) === 5;",
        "const f = (x, y,) => x + y; f(2, 3) === 5 && f.length === 2;",
        "const f = function (x,) { return x; }; f(7) === 7 && f.length === 1;",
        "const o = {f(x,) { return x; }}; o.f(7) === 7;",
        "const f = x => y => x + y; f(2)(3) === 5;",
        "const f = () => ({value: 7}); f().value === 7;",
        "const f = () => {}; f() === undefined;",
        "const f = x => { if (x) { return 7; } return 8; }; f(true) === 7 && f(false) === 8;",
        "((x) => x + 1)(2) === 3;",
        "const f = x => x === 1 || x === 2; f(2) && !f(3);",
        "let n = 0; const f = () => n += 2; f() === 2 && n === 2;",
        "const use = f => f(7); use(x => x + 1) === 8;",
        "const f = x => x; f() === undefined && f(1, 2) === 1;",
        "let f = undefined; f ||= x => x; f.name === 'f' && f.length === 1;",
        "const o = {f: () => 1}; o.f.name === 'f';",
        "class C { f = () => 7; } new C().f.name === 'f';",
        "let n = 1; const f = () => n; n = 2; f() === 2;",
        "const f = (\n x\n) =>\n x; f(7) === 7;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn receivers() {
    for source in [
        "const f = () => this; const o = {f: f}; o.f() === undefined;",
        "const o = {value: 7, make() { return () => this.value; }}; const f = o.make(); const other = {value: 8, f: f}; other.f() === 7;",
        "const o = {value: 7, make() { return () => () => this.value; }}; const f = o.make()(); o.value = 8; f() === 8;",
        "class C { value = 7; f = () => this.value; } const c = new C(); const f = c.f; c.value = 8; f() === 8;",
        "const o = {make() { return () => function () { return this; }; }}; const inner = o.make()(); inner() === undefined;",
        "const o = {make() { return function () { return () => this; }; }}; const other = {make: o.make()}; other.make()() === other;",
        "const C = function () { this.target = () => new.target; }; const c = new C(); c.target() === C;",
        "class A { f = () => new.target; } const a = new A(); a.f() === undefined;",
        "class A { static f = () => this; } const f = A.f; f() === A;",
        "const callbacks = []; for (const value of 'A😀中') { callbacks[callbacks.length] = () => value; } callbacks[0]() === 'A' && callbacks[1]() === '😀';",
        "const f = () => 1; f.prototype === undefined;",
        "const f = () => 1; try { new f(); } catch (e) { e.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn derived() {
    for source in [
        "class A { value = 7; read() { return this.value; } } class B extends A { make() { return () => super.read(); } } const b = new B(); const f = b.make(); b.value = 8; f() === 8;",
        "class A { value = 7; read() { return this.value; } } class B extends A { f = () => super.read(); } const b = new B(); const f = b.f; f() === 7;",
        "class A { static value = 7; static read() { return this.value; } } class B extends A { static make() { return () => super.read(); } } B.value = 8; const f = B.make(); f() === 8;",
        "class A {} class B extends A { constructor() { const read = () => this; let caught = false; try { read(); } catch (e) { caught = e.constructor === ReferenceError; } super(); this.read = read; this.caught = caught; } } const b = new B(); b.caught && b.read() === b;",
        "class A { value = 7; } class B extends A { constructor() { const init = () => super(); init(); this.init = init; } } const b = new B(); b.value === 7;",
        "let calls = 0; class A { constructor() { calls++; } } class B extends A { constructor() { const init = () => super(); init(); try { init(); } catch (e) { this.caught = e.constructor === ReferenceError; } } } const b = new B(); b.caught && calls === 2;",
        "class A { constructor() { this.target = () => new.target; } } class B extends A {} const b = new B(); b.target() === B;",
        "class A {} class B extends A { constructor() { return () => this; } } const f = new B(); try { f(); } catch (e) { e.constructor === ReferenceError; }",
        "class A {} class B extends A { constructor() { return () => super(); } } const init = new B(); const b = init(); b instanceof B;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn syntax() {
    for source in [
        "(x, x) => x;",
        "(,) => 1;",
        "(x,,) => 1;",
        "x => { let x = 1; };",
        "eval => eval;",
        "arguments => arguments;",
        "(true) => 1;",
        "(x + y) => x;",
        "((x)) => x;",
        "x\n=> x;",
        "()/*\n*/=> 1;",
        "1 + x => x;",
        "!x => x;",
        "false || x => x;",
        "() => {}();",
        "() => new.target;",
        "() => super.value;",
        "() => super();",
        "for (const x of [1]) { const f = () => { break; }; }",
        "for (const x of [1]) { const f = () => { continue; }; }",
        "class C { f = () => arguments; }",
        "class C { f = () => () => arguments; }",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
}

#[test]
fn boundaries() {
    let mut engine = Engine::default();
    let stop = engine.native(0);
    engine.register("stop", stop).unwrap();
    let call = engine.native(1);
    engine.register("host", call).unwrap();
    engine.execute("let effects = 0; const f = () => { try { host(() => stop()); } catch { effects = 1; } finally { effects = 2; } };", &mut Host).unwrap();
    assert!(matches!(
        engine.execute("f();", &mut Host),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("effects;", &mut Host).unwrap(),
        Value::Number(0.0)
    );
    let callee = engine.execute("() => 7;", &mut Host).unwrap();
    let foreign = Engine::default().object(Default::default()).unwrap();
    let invocation = Invocation {
        receiver: foreign,
        arguments: vec![],
    };
    assert!(matches!(
        engine.invoke(callee, invocation, &mut Host),
        Err(Failure::Fault(_))
    ));
    assert!(matches!(
        engine.execute("const recurse = () => host(recurse); recurse();", &mut Host),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("(() => 7)();", &mut Host).unwrap(),
        Value::Number(7.0)
    );
}
