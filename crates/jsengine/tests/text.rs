use jsengine::{Access, Addons, Engine, Failure, Invocation, Rejection, Value};

struct Bare;

impl Addons for Bare {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        panic!("text intrinsics must not dispatch to addons")
    }
}

fn evaluate(source: &str) -> Value {
    let mut engine = Engine::default();
    engine
        .execute(source, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)))
}

#[test]
fn scalars() {
    for source in [
        "'😀'.length === 1 && '😀'[0] === '😀' && '😀'[1] === undefined;",
        "'A😀中'.length === 3 && 'A😀中'[1] === '😀' && 'A😀中'[2] === '中';",
        "''.length === 0 && ''[0] === undefined;",
        "'abc'[-1] === undefined && 'abc'['01'] === undefined && 'abc'['-0'] === undefined;",
        "'abc'[0.5] === undefined && 'abc'[99999999999999999] === undefined;",
        "'abc'[-0] === 'a' && 'abc'.absent === undefined;",
        "'e\u{301}'.length === 2 && 'e\u{301}'[1] === '\u{301}';",
        "'👨‍👩‍👧‍👦'.length === 7;",
        "'😀' > '\u{e000}' && '😀' >= '\u{e000}' && '\u{e000}' < '😀';",
        "'😀a' < '😀b' && '😀' < '😀a' && '😀' <= '😀';",
        "'é' !== 'e\u{301}';",
        "const key = '😀'; const o = {[key]: 7}; o['😀'] === 7;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn slicing() {
    for source in [
        "'A😀中'.slice(1, 2) === '😀';",
        "'A😀中'.slice(-2) === '😀中' && 'A😀中'.slice(0, -1) === 'A😀';",
        "'A😀中'.slice() === 'A😀中' && 'A😀中'.slice(1, undefined) === '😀中';",
        "'A😀中'.slice(-99, 99) === 'A😀中';",
        "'A😀中'.slice(2, 1) === '' && 'A😀中'.slice(99) === '';",
        "'A😀中'.slice(-1 / 0, 1 / 0) === 'A😀中';",
        "'A😀中'.slice(0 / 0, 2.9) === 'A😀';",
        "'A😀中'.slice(1 / 0) === '' && 'A😀中'.slice(0, -1 / 0) === '';",
        "'A😀中'.slice('1', null) === '' && 'A😀中'.slice(true) === '😀中';",
        "'A😀中'.slice(-0) === 'A😀中' && ''.slice(-1) === '';",
        "'e\u{301}'.slice(0, 1) === 'e' && 'e\u{301}'.slice(1) === '\u{301}';",
        "'A😀中'.at(1) === '😀' && 'A😀中'.at(-1) === '中';",
        "'A😀中'.at(-3) === 'A' && 'A😀中'.at(-4) === undefined;",
        "'A😀中'.at(3) === undefined && ''.at(0) === undefined;",
        "'A😀中'.at() === 'A' && 'A😀中'.at(1.9) === '😀';",
        "'A😀中'.at(1 / 0) === undefined && 'A😀中'.at(-1 / 0) === undefined;",
        "'A😀中'.at(0 / 0) === 'A';",
        "'A😀中'.codePointAt(1) === 128512 && 'A😀中'.codePointAt(2) === 20013;",
        "'A😀中'.codePointAt(-1) === undefined && 'A😀中'.codePointAt(3) === undefined;",
        "''.codePointAt() === undefined && 'A'.codePointAt() === 65;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn iteration() {
    for source in [
        "let count = 0; let text = ''; for (const value of 'A😀中') { count++; text += value; } count === 3 && text === 'A😀中';",
        "let count = 0; for (const value of 'e\u{301}') { count++; } count === 2;",
        "let count = 0; for (const value of '') { count++; } count === 0;",
        "let source = 'A😀中'; let text = ''; for (const value of source) { source = ''; text += value; } text === 'A😀中';",
        "const callbacks = []; for (const value of 'A😀中') { callbacks[callbacks.length] = function () { return value; }; } callbacks[0]() === 'A' && callbacks[1]() === '😀' && callbacks[2]() === '中';",
        "let text = ''; for (const value of 'A😀中') { if (value === 'A') { continue; } text += value; break; } text === '😀';",
        "const first = function () { for (const value of '😀中') { return value; } }; first() === '😀';",
        "let caught = false; try { for (const value of '😀') { throw value; } } catch (e) { caught = e === '😀'; } caught;",
        "let outer = '😀'; try { for (const outer of outer) {} } catch (e) { e.constructor === ReferenceError; }",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
}

#[test]
fn escapes() {
    for source in [
        r"'\u{1f600}' === '😀';",
        r"'\u{10FFFF}'.codePointAt(0) === 1114111;",
        r"'\u{00000041}' === 'A' && '\u0041' === 'A';",
        r"'\u0000'.length === 1 && '\u0000'.codePointAt(0) === 0;",
        r"'\uD7FF\uE000'.length === 2;",
        r"'\u{FEFF}'.codePointAt(0) === 65279;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    for literal in [
        r"'\uD800'",
        r"'\uDFFF'",
        r"'\uD83D\uDE00'",
        r"'\u{D800}'",
        r"'\u{110000}'",
        r"'\u{FFFFFFFFFFFFFFFF}'",
        r"'\u{}'",
        r"'\u{xyz}'",
        r"'\u{41'",
        r"'\u041'",
        r"'\u-041'",
    ] {
        let source = format!("effects = 1; {literal};");
        assert!(matches!(Engine::check(&source), Err(Rejection::Syntax(_))));
        let mut engine = Engine::default();
        engine.execute("let effects = 0;", &mut Bare).unwrap();
        let error = engine.execute(&source, &mut Bare).unwrap_err();
        assert!(engine.diagnostic(&error).0.contains("SyntaxError"));
        assert_eq!(
            engine.execute("effects;", &mut Bare).unwrap(),
            Value::Number(0.0)
        );
    }
}

#[test]
fn authority() {
    for source in [
        "let effects = 0; try { '😀'[0] = (effects = 1); } catch (e) { effects === 1 && e.constructor === TypeError; }",
        "try { '😀'.length++; } catch (e) { e.constructor === TypeError; }",
        "try { '😀'.extra = 7; } catch (e) { e.constructor === TypeError; }",
        "try { '😀'.slice = 7; } catch (e) { e.constructor === TypeError; }",
        "'😀'.length ||= missing; '😀'.length === 1;",
        "try { const f = '😀'.slice; f(0); } catch (e) { e.constructor === TypeError; }",
        "try { const o = {slice: ''.slice}; o.slice(); } catch (e) { e.constructor === TypeError; }",
        "try { const f = ''.slice; new f(); } catch (e) { e.constructor === TypeError; }",
        "''.slice === '😀'.slice && ''.slice.length === 2 && ''.slice.name === 'slice';",
        "''.at.length === 1 && ''.codePointAt.name === 'codePointAt' && ''.slice.prototype === undefined;",
    ] {
        assert_eq!(evaluate(source), Value::Boolean(true), "{source}");
    }
    let mut engine = Engine::default();
    let text = Value::Text("A😀中".into());
    assert_eq!(engine.get(&text, "length").unwrap(), Value::Number(3.0));
    let method = engine.get(&text, "slice").unwrap();
    let result = engine
        .invoke(
            method.clone(),
            Invocation {
                receiver: text.clone(),
                arguments: vec![Value::Number(1.0), Value::Number(2.0)],
            },
            &mut Bare,
        )
        .unwrap();
    assert_eq!(result, Value::Text("😀".into()));
    assert!(engine.descriptors(&text).is_err());
    assert!(engine.properties(&text).is_err());
    let foreign = Engine::default().object(Default::default()).unwrap();
    let access = Access {
        base: text,
        receiver: foreign,
        key: "length".into(),
    };
    assert!(matches!(
        engine.read(&access, &mut Bare),
        Err(Failure::Fault(_))
    ));
    assert!(matches!(
        Engine::default().call(method, vec![], &mut Bare),
        Err(Failure::Fault(_))
    ));
}

#[test]
fn termination() {
    for method in ["at", "slice", "codePointAt"] {
        let mut engine = Engine::default();
        engine.execute("let effects = 0;", &mut Bare).unwrap();
        let source = format!(
            "try {{ '😀'.{method}({{}}); }} catch {{ effects = 1; }} finally {{ effects = 2; }}"
        );
        assert!(matches!(
            engine.execute(&source, &mut Bare),
            Err(Failure::Fault(_))
        ));
        assert_eq!(
            engine.execute("effects;", &mut Bare).unwrap(),
            Value::Number(0.0)
        );
    }
}
