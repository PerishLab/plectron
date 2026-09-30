mod diagnostic;
mod initializer;
mod location;
use super::native::Bare;
use jsengine::{Engine, Failure, Value};

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine.execute(source, &mut Bare).unwrap_or_else(|failure| {
        panic!("{}: {source}", engine.diagnostic(&failure));
    })
}

#[test]
fn values() {
    for expression in [
        "undefined",
        "null",
        "false",
        "12",
        "'payload'",
        "({x: 1})",
        "(function () {})",
    ] {
        let source = format!(
            "const value = {expression}; let seen = false; try {{ throw value; }} catch (error) {{ seen = error === value; }} seen;"
        );
        assert_eq!(evaluate(&source), Value::Boolean(true));
    }
    assert_eq!(
        evaluate(
            "const fail = function () { throw 42; }; try { fail(); } catch (error) { error; }"
        ),
        Value::Number(42.0)
    );
    let mut engine = Engine::default();
    let failure = engine
        .execute("const value = {}; throw value;", &mut Bare)
        .unwrap_err();
    let Failure::Throw(value) = failure else {
        panic!("expected throw")
    };
    assert_eq!(value, engine.execute("value;", &mut Bare).unwrap());
}

#[test]
fn catches() {
    assert_eq!(
        evaluate(
            "let error = 'outer'; let saved = undefined; try { throw 'inner'; } catch (error) { saved = function () { return error; }; error = 'changed'; } error + saved();"
        ),
        Value::Text("outerchanged".into())
    );
    assert_eq!(
        evaluate(
            "let value = 0; try { throw 1; } catch { let value = 2; } finally { value = 3; } value;"
        ),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate(
            "try { try { throw 1; } catch (error) { throw error + 1; } } catch (error) { error; }"
        ),
        Value::Number(2.0)
    );
    assert_eq!(
        evaluate("try { try { throw 1; } finally { throw 2; } } catch (error) { error; }"),
        Value::Number(2.0)
    );
}

#[test]
fn finalization() {
    for (body, expected) in [
        ("try { return 1; } finally { 2; }", 1.0),
        ("try { return 1; } finally { return 2; }", 2.0),
        ("try { throw 1; } finally { return 2; }", 2.0),
        (
            "try { throw 1; } catch (error) { return error; } finally { 2; }",
            1.0,
        ),
        (
            "try { throw 1; } catch (error) { return error; } finally { return 2; }",
            2.0,
        ),
        (
            "for (let n of [1]) { try { break; } finally { return 2; } }",
            2.0,
        ),
        (
            "for (let n of [1]) { try { continue; } finally { return 2; } }",
            2.0,
        ),
    ] {
        assert_eq!(
            evaluate(&format!("(function () {{ {body} }})();")),
            Value::Number(expected),
            "{body}"
        );
    }
    assert_eq!(
        evaluate(
            "let count = 0; for (let n of [1, 2, 3]) { try { if (n === 2) { break; } continue; } finally { count = count + n; } } count;"
        ),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate(
            "let count = 0; for (let n of [1, 2]) { try { throw 9; } finally { count = count + n; continue; } } count;"
        ),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate(
            "try { (function () { try { return 1; } finally { throw 2; } })(); } catch (error) { error; }"
        ),
        Value::Number(2.0)
    );
}

#[test]
fn completion() {
    for (source, expected) in [
        ("1; {};", Value::Number(1.0)),
        ("1; let value = 2;", Value::Number(1.0)),
        ("1; try {} finally {}", Value::Undefined),
        ("try { 1; } finally { 2; }", Value::Number(1.0)),
        ("try { throw 1; } catch {}", Value::Undefined),
        (
            "try { throw 1; } catch { 2; } finally { 3; }",
            Value::Number(2.0),
        ),
        ("for (let n of [1]) { 3; break; }", Value::Number(3.0)),
        (
            "for (let n of [1]) { 3; try { break; } finally {} }",
            Value::Undefined,
        ),
        (
            "for (let n of [1]) { try { 3; break; } finally { 4; } }",
            Value::Number(3.0),
        ),
        (
            "for (let n of [1]) { try { 3; continue; } finally { 4; } }",
            Value::Number(3.0),
        ),
        ("1; if (true) {}", Value::Undefined),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
}

#[test]
fn errors() {
    for (body, kind) in [
        ("missing;", "ReferenceError"),
        ("missing = 1;", "ReferenceError"),
        ("let x = x;", "ReferenceError"),
        ("const x = (x = 1);", "ReferenceError"),
        ("let x = 1; for (const x of [x]) {}", "ReferenceError"),
        ("const x = 1; x = 2;", "TypeError"),
        ("null();", "TypeError"),
        ("null.x;", "TypeError"),
        ("null.x = 1;", "TypeError"),
        ("const x = []; x.length = -1;", "RangeError"),
        ("for (let x of 1) {}", "TypeError"),
    ] {
        let source = format!("try {{ {body} }} catch (error) {{ error.constructor === {kind}; }}");
        assert_eq!(evaluate(&source), Value::Boolean(true), "{body}");
    }
    for kind in [
        "Error",
        "TypeError",
        "ReferenceError",
        "SyntaxError",
        "RangeError",
    ] {
        let source = format!(
            "const one = new {kind}('message'); const two = {kind}(); one !== two && one.constructor === {kind} && one.name === '{kind}' && one.message === 'message' && two.message === '' && one instanceof {kind} && one instanceof Error;"
        );
        assert_eq!(evaluate(&source), Value::Boolean(true));
    }
    assert_eq!(
        evaluate(
            "let mark = 0; const fail = function () { throw 7; }; try { fail() + (mark = 1); } catch {} mark;"
        ),
        Value::Number(0.0)
    );
}

#[test]
fn parsing() {
    for source in [
        "try {}",
        "try 1; catch {}",
        "try {} catch () {}",
        "try {} catch (eval) {}",
        "try {} catch (e) { let e = 1; }",
        "throw;",
        "throw\n1;",
        "throw /*\n*/ 1;",
        "try {} finally { break; }",
        "try {} finally { return 1; }",
        "for (let n of [1]) { (function () { try {} finally { continue; } }); }",
    ] {
        let mut engine = Engine::default();
        let failure = engine.execute(source, &mut Bare).unwrap_err();
        let Failure::Throw(value) = failure else {
            panic!("expected SyntaxError: {source}")
        };
        assert_eq!(
            engine.get(&value, "name").unwrap(),
            Value::Text("SyntaxError".into()),
            "{source}"
        );
    }
    let mut engine = Engine::default();
    engine.execute("let marker = 0;", &mut Bare).unwrap();
    assert!(
        engine
            .execute("marker = 1; try {} catch (e) { const e = 1; }", &mut Bare)
            .is_err()
    );
    assert_eq!(
        engine.execute("marker;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
}

#[test]
fn identity() {
    assert_eq!(
        evaluate(
            "TypeError.prototype.constructor === TypeError && TypeError.prototype instanceof Error && !(Error() instanceof TypeError);"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "Error.prototype.tag = 7; const error = new TypeError(); error.tag === 7 && error.constructor === TypeError;"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate(
            "try { TypeError.prototype = {}; } catch (error) { error.constructor === TypeError; }"
        ),
        Value::Boolean(true)
    );
    assert_eq!(
        evaluate("const namespace = {Error: Error}; new namespace['Error']('x').message;"),
        Value::Text("x".into())
    );
    let mut engine = Engine::default();
    engine.execute("let marker = 0;", &mut Bare).unwrap();
    let failure = engine
        .execute("marker = 1; let marker = 2;", &mut Bare)
        .unwrap_err();
    let Failure::Throw(value) = failure else {
        panic!("expected SyntaxError")
    };
    assert_eq!(
        engine.get(&value, "name").unwrap(),
        Value::Text("SyntaxError".into())
    );
    assert_eq!(
        engine.execute("marker;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
}
