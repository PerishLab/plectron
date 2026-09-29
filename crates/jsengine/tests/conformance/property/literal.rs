use crate::native::Bare;
use jsengine::{Content, Engine, Rejection, Value};

#[test]
fn expressions() {
    for source in [
        "const o = {n: 1, get value() { return this.n; }, set value(n) { this.n = n; }}; o.value = 7; o.value === 7;",
        "const o = {method() { return this; }}; o.method() === o && o.method.prototype === undefined && o.method.name === 'method';",
        "const o = {constructor() { return this; }}; o.constructor() === o;",
        "const o = {method() {}}; try { new o.method(); } catch (e) { e.constructor === TypeError; }",
        "const o = {get value() { return 1; }, set value(n) { this.n = n; }, get value() { return this.n; }}; o.value = 7; o.value === 7;",
        "const o = {get value() { throw 1; }, value: 7}; o.value === 7;",
        "const o = {set value(n) { throw 1; }, value: 7, get value() { return 8; }}; try { o.value = 9; } catch (e) { o.value === 8 && e.constructor === TypeError; }",
        "const o = {get: 7, set() { return 8; }, 'get'() { return 9; }}; o.get() === 9 && o.set() === 8;",
        "let order = ''; const key = function (s) { order = order + s; return s; }; const o = {[key('a')]: (order = order + 'v'), get [key('b')]() { return 7; }, [key('c')]() { return 8; }}; order === 'avbc' && o.b === 7 && o.c() === 8;",
        "const o = {['__proto__']: 7,}; o.__proto__ === 7;",
        "const o = {get __proto__() { return 7; }}; o.__proto__ === 7;",
        "const o = {__proto__() { return 7; }}; o.__proto__() === 7;",
        "const o = {method() { return new.target; }, get value() { return new.target; }}; o.method() === undefined && o.value === undefined;",
        "const o = {method() { return super.missing; }}; o.method() === undefined;",
    ] {
        let mut engine = Engine::default();
        let value = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(value, Value::Boolean(true), "{source}");
    }
}

#[test]
fn descriptors() {
    let mut engine = Engine::default();
    let value = engine
        .execute(
            "({method() {}, get value() { return 7; }, set value(n) {}});",
            &mut Bare,
        )
        .unwrap();
    for name in ["method", "value"] {
        let property = engine.descriptor(&value, name).unwrap().unwrap();
        assert!(property.enumerable && property.configurable);
    }
    assert!(matches!(
        engine.descriptor(&value, "value").unwrap().unwrap().content,
        Content::Accessor {
            get: Some(_),
            set: Some(_)
        }
    ));
    assert!(engine.properties(&value).is_err());
    let prototype = engine.prototype(&value).unwrap();
    let method = engine
        .execute("(function () { return this.n; });", &mut Bare)
        .unwrap();
    engine.set(&prototype, "read", method).unwrap();
    assert_eq!(engine.execute("const o = {n: 7, read() { return super.read(); }}; const other = {n: 8, read: o.read}; other.read();", &mut Bare).unwrap(), Value::Number(8.0));
}

#[test]
fn rejection() {
    for source in [
        "({get value(n) {}});",
        "({set value() {}});",
        "({set value(a, b) {}});",
        "({'get' value() {}});",
        "({method() { super(); }});",
        "({__proto__: {}});",
        "({method: function () { return super.value; }});",
    ] {
        assert!(
            matches!(Engine::check(source), Err(Rejection::Syntax(_))),
            "{source}"
        );
    }
}
