use super::native::Bare;
use jsengine::{Engine, Value};

fn evaluate(source: &str) -> Value {
    Engine::default().execute(source, &mut Bare).unwrap()
}

fn reject(source: &str, note: &str) {
    let mut engine = Engine::default();
    let failure = engine.execute(source, &mut Bare).unwrap_err();
    let error = engine.diagnostic(&failure);
    assert!(error.0.contains(note), "{error}");
}

#[test]
fn arrays() {
    assert_eq!(evaluate("[1, 2, 3,].length;"), Value::Number(3.0));
    assert_eq!(evaluate("[, , 4][0];"), Value::Undefined);
    assert_eq!(evaluate("[, , 4].length;"), Value::Number(3.0));
    assert_eq!(
        evaluate("const a = [1]; const b = a; b[0] = 7; a[0];"),
        Value::Number(7.0)
    );
    assert_eq!(
        evaluate("const a = []; a[3] = 9; a.length;"),
        Value::Number(4.0)
    );
    assert_eq!(
        evaluate("const a = [1, 2]; a.length = 1; a.length = 2; a[1];"),
        Value::Undefined
    );
    assert_eq!(
        evaluate("const a = []; a['01'] = 5; a[-1] = 4; a[4294967295] = 3; a.length;"),
        Value::Number(0.0)
    );
    assert_eq!(
        evaluate("const a = []; a[4294967294] = 5; a.length;"),
        Value::Number(4294967295.0)
    );
    reject("const a = []; a.length = -1;", "invalid array length");
    reject("const a = []; a.length = 1.5;", "invalid array length");
    reject(
        "const a = []; a.length = 4294967296;",
        "invalid array length",
    );
}

#[test]
fn weak() {
    for source in [
        "const key = {}; const map = new WeakMap(); map.set(key, 7) === map && map.get(key) === 7 && map.has(key) && map.delete(key) && !map.has(key);",
        "const key = {}; const set = new WeakSet(); set.add(key) === set && set.has(key) && set.delete(key) && !set.has(key);",
        "const map = new WeakMap(); map.get(1) === undefined && map.has(1) === false && map.delete(1) === false;",
        "const set = new WeakSet(); set.has(1) === false && set.delete(1) === false;",
        "const map = new WeakMap(); map.size === undefined && map[Symbol.iterator] === undefined;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    reject("WeakMap();", "requires new");
    reject("new WeakMap([[]]);", "iterable is unsupported");
    reject("new WeakMap().set(1, 2);", "key must be an object");
    reject("new WeakSet().add(Symbol('key'));", "key must be an object");
    reject(
        "WeakMap.prototype.get({});",
        "invalid weak collection receiver",
    );
    reject(
        "WeakSet.prototype.add({});",
        "invalid weak collection receiver",
    );
}

#[test]
fn arrayintrinsic() {
    for source in [
        "Array.isArray([]) && Array.isArray(Array(2)) && Array.isArray(new Array(1));",
        "const a = new Proxy([1], {}); Array.isArray(a) && !Array.isArray({length: 1});",
        "const a = Array(2); a.length === 2 && !(0 in a) && !(1 in a);",
        "const a = Array(1, 2); a.length === 2 && a[0] === 1 && a[1] === 2;",
        "const pair = Proxy.revocable([], {}); pair.revoke(); try { Array.isArray(pair.proxy); false; } catch (error) { error instanceof TypeError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    reject("Array(1.5);", "invalid array length");
}

#[test]
fn properties() {
    assert_eq!(
        evaluate("const a = {0: 1, 'for': 2}; a['value'] = a.for + a[0]; a.value;"),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate("const a = {}; const b = {}; a.x = b.x = 5; a.x + b.x;"),
        Value::Number(10.0)
    );
    assert_eq!(
        evaluate("const a = {}; a.self = a; a.self === a;"),
        Value::Boolean(true)
    );
    reject("const a = {}; a[{}] = 1;", "key coercion");
    reject("1.x = 1;", "expected ;");
    reject("(1).x = 1;", "non-object");
    reject("(1 + 2) = 4;", "assignment target");
}

#[test]
fn ordering() {
    assert_eq!(
        evaluate(
            "let trace = ''; const a = {}; const base = function () { trace = trace + 'b'; return a; }; const key = function () { trace = trace + 'k'; return 'x'; }; const rhs = function () { trace = trace + 'v'; return 4; }; base()[key()] = rhs(); trace + a.x;"
        ),
        Value::Text("bkv4".into())
    );
    let mut engine = Engine::default();
    engine.execute("let n = 0;", &mut Bare).unwrap();
    assert!(engine.execute("null.x = (n = 1);", &mut Bare).is_err());
    assert_eq!(engine.execute("n;", &mut Bare).unwrap(), Value::Number(1.0));
    assert!(engine.execute("missing = (n = 2);", &mut Bare).is_err());
    assert_eq!(engine.execute("n;", &mut Bare).unwrap(), Value::Number(2.0));
}

#[test]
fn iteration() {
    assert_eq!(
        evaluate("let total = 0; for (const value of [1, 2, 3]) { total = total + value; } total;"),
        Value::Number(6.0)
    );
    assert_eq!(
        evaluate(
            "let total = 0; for (let value of [1, 2]) { value = value * 2; total = total + value; } total;"
        ),
        Value::Number(6.0)
    );
    assert_eq!(
        evaluate(
            "let n = 0; for (const value of [,,]) { if (value === undefined) { n = n + 1; } } n;"
        ),
        Value::Number(2.0)
    );
    reject("for (const value of [1]) { value = 2; }", "const");
    reject("for (const value of []) {} value;", "unknown binding");
    reject(
        "let value = [1]; for (const value of value) {}",
        "not initialized",
    );
    reject("for (const value of {}) {}", "only arrays and text");
}

#[test]
fn iteratorprotocol() {
    for source in [
        "const a = [2, 3]; const iterator = a[Symbol.iterator](); iterator[Symbol.iterator]() === iterator && iterator.next().value === 2 && iterator.next().value === 3 && iterator.next().done === true;",
        "const a = [1, 2]; a[Symbol.iterator] = function () { let n = 4; return {next() { n++; return {done: n > 6, value: n}; }}; }; let total = 0; for (const value of a) { total += value; } total === 11;",
        "const source = {[Symbol.iterator]() { let n = 0; return {next() { n++; return {done: n > 2, value: n}; }}; }}; const values = [...source]; values[0] === 1 && values[1] === 2 && values.length === 2;",
        "let closed = 0; const source = {[Symbol.iterator]() { return {next() { return {done: false, value: 1}; }, return() { closed++; return {}; }}; }}; for (const value of source) { break; } closed === 1;",
        "let closed = 0; const source = {[Symbol.iterator]() { return {next() { return {done: false, value: 1}; }, return() { closed++; return {}; }}; }}; const first = () => { for (const value of source) { return value; } }; first() === 1 && closed === 1;",
        "let closed = 0; const source = {[Symbol.iterator]() { return {next() { return {done: false, value: 1}; }, return() { closed++; return {}; }}; }}; try { for (const value of source) { throw 7; } } catch (error) { if (error !== 7) { throw error; } } closed === 1;",
        "const iterator = 'A😀中'[Symbol.iterator](); iterator.next().value === 'A' && iterator.next().value === '😀' && iterator.next().value === '中' && iterator.next().done;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    reject(
        "const a = [1]; a[Symbol.iterator] = 3; for (const x of a) {}",
        "callable",
    );
    reject(
        "const a = [1]; a[Symbol.iterator] = () => 3; for (const x of a) {}",
        "iterator must be an object",
    );
}

#[test]
fn proxyiteration() {
    assert_eq!(
        evaluate(
            "const reads = []; const proxy = new Proxy([1, 2], {get(target, key, receiver) { reads[reads.length] = key; return Reflect.get(target, key, receiver); }}); let total = 0; for (const value of proxy) { total += value; } total === 3 && reads[0] === Symbol.iterator && reads[1] === 'length' && reads[2] === '0' && reads[3] === 'length' && reads[4] === '1' && reads[5] === 'length';"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "const proxy = new Proxy([1, 2], {}); const values = [...proxy]; values[0] === 1 && values[1] === 2;"
        ),
        Value::Boolean(true)
    );
}

#[test]
fn capture() {
    assert_eq!(
        evaluate(
            "const callbacks = []; for (const value of [1, 2, 3]) { callbacks[callbacks.length] = function () { return value; }; } callbacks[0]() + callbacks[2]();"
        ),
        Value::Number(4.0)
    );
    assert_eq!(
        evaluate(
            "const callbacks = []; for (let value of [1, 2]) { callbacks[callbacks.length] = function () { value = value + 10; return value; }; } callbacks[0](); callbacks[1](); callbacks[0]();"
        ),
        Value::Number(21.0)
    );
}

#[test]
fn control() {
    assert_eq!(
        evaluate(
            "let n = 0; for (const value of [1, 2, 3, 4]) { if (value === 2) { continue; } if (value === 4) { break; } n = n + value; } n;"
        ),
        Value::Number(4.0)
    );
    assert_eq!(
        evaluate(
            "const f = function () { for (const value of [1, 2]) { for (const other of [3]) { return value + other; } } }; f();"
        ),
        Value::Number(4.0)
    );
    reject("break;", "outside loop");
    reject(
        "for (const x of [1]) { const f = function () { continue; }; }",
        "outside loop",
    );
    reject("for (const x of [1]) let a = 1;", "needs a block");
}

#[test]
fn mutation() {
    assert_eq!(
        evaluate(
            "const a = [1]; let n = 0; for (const value of a) { n = n + value; if (value === 1) { a[1] = 2; } } n;"
        ),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate(
            "const a = [1, 2, 3]; let n = 0; for (const value of a) { n = n + 1; a.length = 1; } n;"
        ),
        Value::Number(1.0)
    );
    reject(
        "const a = [1]; for (const value of a) { a[a.length] = value; }",
        "budget",
    );
}

#[test]
fn isolation() {
    let mut engine = Engine::default();
    let array = engine.array(vec![Some(Value::Number(1.0))]).unwrap();
    let mut other = Engine::default();
    let foreign = other.array(vec![]).unwrap();
    assert!(other.get(&array, "length").is_err());
    assert!(other.set(&array, "0", Value::Null).is_err());
    assert!(engine.set(&array, "0", foreign.clone()).is_err());
    assert!(engine.array(vec![Some(foreign)]).is_err());
    assert_eq!(engine.get(&array, "0").unwrap(), Value::Number(1.0));
}

#[test]
fn strictness() {
    for source in [
        "let eval = 1;",
        "const arguments = 1;",
        "const f = function (eval) {};",
        "eval = 1;",
    ] {
        assert!(
            Engine::default().execute(source, &mut Bare).is_err(),
            "{source}"
        );
    }
    reject("implicit = 1;", "unknown binding");
    reject("with ({}) {}", "unsupported keyword");
}

#[test]
fn frozen() {
    let mut engine = Engine::default();
    let value = engine.array(vec![Some(Value::Number(7.0))]).unwrap();
    engine.freeze(&value).unwrap();
    for key in ["0", "length", "extra"] {
        assert!(engine.set(&value, key, Value::Number(0.0)).is_err());
    }
    assert_eq!(engine.get(&value, "0").unwrap(), Value::Number(7.0));
    assert_eq!(engine.get(&value, "length").unwrap(), Value::Number(1.0));
    assert!(Engine::default().freeze(&value).is_err());
}

#[test]
fn checking() {
    assert!(Engine::check("missing();").is_ok());
    assert!(Engine::check("const a = []; a[0] = 1;").is_ok());
    assert!(Engine::check("(1 + 2) = 3;").is_err());
}
