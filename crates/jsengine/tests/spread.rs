use jsengine::{Addons, Engine, Failure, Invocation, Rejection, Value};

struct Host;

impl Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, invocation: Invocation) -> Result<Value, Failure> {
        Ok(Value::Number(invocation.arguments.len() as f64))
    }
}

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Host)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn arrays() {
    for source in [
        "const a = [0, ...[1, 2], 3]; a.length === 4 && a[1] === 1 && a[3] === 3;",
        "const a = [...[], ...'A😀中']; a.length === 3 && a[1] === '😀';",
        "const a = [...'é']; a.length === 2 && a[0] === 'e';",
        "const a = [...'', ...[]]; a.length === 0;",
        "const a = [...[, 7],, ...[9],]; a.length === 4 && a[0] === undefined && a[3] === 9;",
        "const o = {}; const a = [o]; const b = [...a]; a !== b && b[0] === o;",
        "const a = [1]; const b = [...a]; a[0] = 2; b[0] === 1;",
        "const a = [1]; const b = [...a, a[0] = 2, ...a]; b[0] === 1 && b[1] === 2 && b[2] === 2;",
        "const a = [...[...[1, 2]], ...[3]]; a.length === 3;",
        "const f = (a = [...'😀']) => a; f()[0] === '😀';",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    let mut engine = Engine::default();
    let array = engine.execute("[0, ...[, 7],, 9];", &mut Host).unwrap();
    let slot = engine.descriptor(&array, "1").unwrap().unwrap();
    assert_eq!(slot.value().unwrap(), Value::Undefined);
    assert!(slot.enumerable && slot.configurable && slot.writable() == Some(true));
    assert_eq!(engine.descriptor(&array, "3").unwrap(), None);
}

#[test]
fn calls() {
    for source in [
        "const f = (...items) => items; const a = f(0, ...[1, 2], 3,); a.length === 4 && a[2] === 2;",
        "const f = (...items) => items.length; f(...[],) === 0 && f(...'A😀') === 2;",
        "const f = (a = 1, b = 2, ...rest) => a + b + rest.length; f(...[, , 7]) === 4;",
        "const f = (...items) => [...items]; const forward = (...items) => f(...items); forward(1, 2)[1] === 2;",
        "const o = {value: 7, f(a) { return this.value + a; }}; o.f(...[1]) === 8;",
        "'A😀中'.slice(...[1, 2]) === '😀';",
        "const o = {f() { return 1; }}; const change = () => { o.f = () => 2; return []; }; o.f(...change()) === 1;",
        "let order = ''; const o = {get f() { order += 'g'; return (...items) => { order += 'c'; return items.length; }; }}; const source = () => { order += 's'; return [1, 2]; }; const n = o.f(...source()); n === 2 && order === 'gsc';",
        "let calls = 0; const source = () => { calls++; return [1]; }; const f = (...items) => items.length; f(...source(), ...source()) === 2 && calls === 2;",
        "let effects = 0; const f = (...items) => { effects = 2; }; const source = () => { effects = 1; throw 7; }; try { f(...source()); } catch (e) { e === 7 && effects === 1; }",
        "let effects = 0; try { (1)(...(effects = [])); } catch (e) { e.constructor === TypeError && effects.length === 0; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    let mut engine = Engine::default();
    let native = engine.native(0);
    engine.register("native", native).unwrap();
    assert_eq!(
        engine
            .execute("native(...[1, 2], ...'😀');", &mut Host)
            .unwrap(),
        Value::Number(3.0)
    );
}

#[test]
fn construction() {
    for source in [
        "const F = function (...items) { this.items = items; }; new F(...[1, 2]).items.length === 2;",
        "class A { constructor(...items) { this.items = items; this.target = new.target; } } class B extends A { constructor(...items) { super(...items); } } const b = new B(...[1, 2]); b.items[1] === 2 && b.target === B;",
        "class A { constructor(a = 7) { this.value = a; } } class B extends A { constructor(...items) { super(...items); } } new B(...[]).value === 7;",
        "class A { f(a, b) { return this.value + a + b; } } class B extends A { value = 1; f(...items) { return super.f(...items); } } new B().f(...[2, 3]) === 6;",
        "class A { constructor(...items) { this.items = items; } } class B extends A { constructor(...items) { const init = () => super(...items); init(); } } new B(...'😀').items[0] === '😀';",
        "let effects = 0; const f = () => 1; const source = () => { effects++; return []; }; try { new f(...source()); } catch (e) { effects === 1 && e.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn refusal() {
    for source in [
        "try { [...null]; } catch (e) { e.constructor === TypeError; }",
        "try { [...undefined]; } catch (e) { e.constructor === TypeError; }",
        "try { [...1]; } catch (e) { e.constructor === TypeError; }",
        "let effects = 0; try { [...{}, effects = 1]; } catch (e) { effects === 0 && e.constructor === TypeError; }",
        "let effects = 0; const o = {get length() { effects++; return 1; }, 0: 7}; try { [...o]; } catch (e) { effects === 0 && e.constructor === TypeError; }",
        "let effects = 0; const f = (a = (effects = 1)) => 7; try { f(...false); } catch (e) { effects === 0 && e.constructor === TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    for source in [
        "[...];",
        "f(...);",
        "f(,);",
        "f(1,,2);",
        "(...items,) => 0;",
        "const x = ...[];",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
}

#[test]
fn budget() {
    let mut engine = Engine::default();
    let array = engine.array(vec![]).unwrap();
    engine.set(&array, "length", Value::Number(4096.0)).unwrap();
    engine.register("large", array.clone()).unwrap();
    engine
        .execute(
            "let effects = 0; const count = (...items) => items.length;",
            &mut Host,
        )
        .unwrap();
    assert_eq!(
        engine
            .execute("count(...large, ...[],);", &mut Host)
            .unwrap(),
        Value::Number(4096.0)
    );
    assert_eq!(
        engine
            .execute("[...large, ...[]].length;", &mut Host)
            .unwrap(),
        Value::Number(4096.0)
    );
    let result = engine.execute(
        "try { count(...large, effects = 1); } catch { effects = 2; } finally { effects = 3; }",
        &mut Host,
    );
    assert!(matches!(result, Err(Failure::Fault(_))));
    assert_eq!(
        engine.execute("effects;", &mut Host).unwrap(),
        Value::Number(1.0)
    );
    engine
        .set(&array, "length", Value::Number(100_000.0))
        .unwrap();
    let result = engine.execute("[...large];", &mut Host);
    assert!(matches!(result, Err(Failure::Fault(_))));
    assert_eq!(
        engine.execute("[...'😀'].length;", &mut Host).unwrap(),
        Value::Number(1.0)
    );
}
