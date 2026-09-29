use jsengine::{Addons, Engine, Failure, Fault, Invocation, Rejection, Value};
struct Host;
impl Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        Err(Fault("host stopped".into()).into())
    }
}
fn check(source: &str, expected: &str) {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Host)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
    engine.checkpoint(&mut Host).unwrap();
    assert_eq!(
        engine.execute("result;", &mut Host).unwrap().text(),
        expected,
        "{source}"
    );
    assert!(engine.jobs().rejected.is_empty(), "{:?}", engine.jobs());
}
#[test]
fn promises() {
    for (source, expected) in [
        (
            "let result = ''; new Promise((resolve, reject) => { result += 'a'; resolve(7); reject(8); }).then(x => { result += x; }); result += 'b';",
            "ab7",
        ),
        (
            "let result = 0; Promise.resolve(2).then(x => x * 3).finally(() => Promise.resolve(99)).then(x => { result = x; });",
            "6",
        ),
        (
            "let result = ''; Promise.reject('a').finally(() => { result += 'f'; }).catch(x => { result += x; });",
            "fa",
        ),
        (
            "let result = 0; Promise.resolve({then(resolve, reject) { resolve(8); reject(9); throw 10; }}).then(x => { result = x; });",
            "8",
        ),
        (
            "let result = 0; Promise.resolve({get then() { throw 7; }}).catch(x => { result = x; });",
            "7",
        ),
        (
            "let result = false; const p = Promise.resolve(1); const q = p.then(() => q); q.catch(e => { result = e.constructor === TypeError; });",
            "true",
        ),
        (
            "let result = false; const p = Promise.resolve(1); result = Promise.resolve(p) === p;",
            "true",
        ),
    ] {
        check(source, expected);
    }
}
#[test]
fn continuations() {
    for (source, expected) in [
        (
            "let result = ''; const f = async () => { result += 'a'; const x = await 7; result += x; }; f(); result += 'b';",
            "ab7",
        ),
        (
            "let result = 0; const f = async x => 2 + await Promise.resolve(x); f(5).then(x => { result = x; });",
            "7",
        ),
        (
            "let result = 0; const f = async function () { try { await Promise.reject(7); } catch (e) { return e; } finally { await 2; } }; f().then(x => { result = x; });",
            "7",
        ),
        (
            "let result = 0; const f = async () => { try { return await 7; } finally { return await 9; } }; f().then(x => { result = x; });",
            "9",
        ),
        (
            "let result = ''; const f = async () => { for (const x of [1, 2, 3]) { try { if (x === 2) { continue; } result += await x; } finally { result += await 'f'; } } }; f();",
            "1ff3f",
        ),
        (
            "let result = 0; const f = async () => { const a = [await 1, ...(await [2, 3])]; const b = {[await 'x']: await 4}; return a[0] + a[2] + b.x; }; f().then(x => { result = x; });",
            "8",
        ),
        (
            "let result = 0; const object = {x: 7, f(v) { return this.x + v; }}; const f = async () => object[await 'f'](await 3); f().then(x => { result = x; });",
            "10",
        ),
        (
            "let result = 0; let object = {x: 2}; const before = object; const f = async () => { object.x += await Promise.resolve(3); }; f(); object = {x: 99}; Promise.resolve().then(() => { result = before.x; });",
            "5",
        ),
        (
            "let result = 0; const f = async () => { false && await Promise.reject(1); true || await Promise.reject(2); return 7; }; f().then(x => { result = x; });",
            "7",
        ),
    ] {
        check(source, expected);
    }
}
#[test]
fn ownership() {
    let mut engine = Engine::default();
    let deferred = engine.deferred().unwrap();
    engine.register("input", deferred.promise).unwrap();
    engine
        .execute(
            "let result = 0; const f = async () => { result = await input; }; f();",
            &mut Host,
        )
        .unwrap();
    assert_eq!(engine.jobs().queued, 0);
    engine.checkpoint(&mut Host).unwrap();
    assert_eq!(
        engine.execute("result;", &mut Host).unwrap(),
        Value::Number(0.0)
    );
    let foreign = Engine::default().object(Default::default()).unwrap();
    assert!(
        engine
            .call(deferred.resolve.clone(), vec![foreign], &mut Host)
            .is_err()
    );
    engine
        .call(deferred.resolve, vec![Value::Number(8.0)], &mut Host)
        .unwrap();
    engine.checkpoint(&mut Host).unwrap();
    assert_eq!(
        engine.execute("result;", &mut Host).unwrap(),
        Value::Number(8.0)
    );
    let stop = engine.native(0);
    engine.register("stop", stop).unwrap();
    engine
        .execute("Promise.resolve().then(() => stop());", &mut Host)
        .unwrap();
    assert!(engine.checkpoint(&mut Host).is_err());
}
#[test]
fn syntax() {
    for source in [
        "await 1;",
        "const f = async (x = await 1) => x;",
        "const f = async () => { const g = () => await 1; };",
        "const f = async\n() => 1;",
        "const f = async () => { const g = function () { await 1; }; };",
        "const f = async (x, x) => x;",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
}

#[test]
fn addresses() {
    let mut engine = Engine::default();
    engine
        .execute(
            "let target = 1; const run = async () => { target += await 5; }; run();",
            &mut Host,
        )
        .unwrap();
    engine.execute("for (const a of [0,1,2,3,4,5,6,7]) { for (const b of [0,1,2,3,4,5,6,7]) { let local = a + b; } } target = 100;", &mut Host).unwrap();
    engine.checkpoint(&mut Host).unwrap();
    assert_eq!(
        engine.execute("target;", &mut Host).unwrap(),
        Value::Number(6.0)
    );
    assert!(engine.jobs().rejected.is_empty());
}
#[test]
fn references() {
    let mut engine = Engine::default();
    engine.execute("const original = {x: 1}; let target = original; let key = 'x'; let result = 0; const run = async () => { target[key] += await 2; result = original.x; }; run();", &mut Host).unwrap();
    engine
        .execute("target = {x: 100}; key = 'y';", &mut Host)
        .unwrap();
    engine.checkpoint(&mut Host).unwrap();
    assert_eq!(
        engine
            .execute(
                "result === 3 && target.x === 100 && original.y === undefined;",
                &mut Host
            )
            .unwrap(),
        Value::Boolean(true)
    );
    assert!(engine.jobs().rejected.is_empty());
}
