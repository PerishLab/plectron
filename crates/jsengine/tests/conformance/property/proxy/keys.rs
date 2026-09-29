use super::{check, setup};
use jsengine::{Atom, Content, Property, Value};

#[test]
fn keys() {
    let (mut engine, target, _) = setup("({x: 1})", "({ownKeys() { return ['virtual']; }})");
    check(
        &mut engine,
        "const keys = Reflect.ownKeys(proxy); keys.length === 1 && keys[0] === 'virtual';",
    );
    engine.prevent(&target).unwrap();
    check(
        &mut engine,
        "try { Reflect.ownKeys(proxy); false; } catch (e) { e instanceof TypeError; }",
    );
}

#[test]
fn keyrules() {
    for handler in [
        "({ownKeys() { return ['x', 'x']; }})",
        "({ownKeys() { return [1]; }})",
        "({ownKeys() { return 1; }})",
    ] {
        let (mut engine, _, _) = setup("({x: 1})", handler);
        check(
            &mut engine,
            "try { Reflect.ownKeys(proxy); false; } catch (e) { e instanceof TypeError; }",
        );
    }
}

#[test]
fn keyforward() {
    let (mut engine, _, _) = setup("({x: 1})", "({})");
    check(
        &mut engine,
        "const keys = Reflect.ownKeys(proxy); keys.length === 1 && keys[0] === 'x';",
    );
}

#[test]
fn keyfixed() {
    let (mut engine, target, _) = setup("({x: 1})", "({ownKeys() { return []; }})");
    engine
        .configure(
            &target,
            &Atom::String("x".into()),
            Property {
                content: Content::Data {
                    value: Value::Number(1.0),
                    writable: true,
                },
                enumerable: true,
                configurable: false,
            },
        )
        .unwrap();
    check(
        &mut engine,
        "try { Reflect.ownKeys(proxy); false; } catch (e) { e instanceof TypeError; }",
    );
}
