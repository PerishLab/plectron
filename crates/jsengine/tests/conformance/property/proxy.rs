mod builtin;
mod invocation;
mod keys;

use crate::native::Bare;
use jsengine::{Atom, Content, Engine, Property, Value};

fn setup(target: &str, handler: &str) -> (Engine, Value, Value) {
    let mut engine = Engine::default();
    let target = engine
        .execute(&format!("{target};"), &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {target}", engine.diagnostic(&failure)));
    let handler = engine
        .execute(&format!("{handler};"), &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {handler}", engine.diagnostic(&failure)));
    let proxy = engine.proxy(&target, &handler).unwrap();
    engine.register("target", target.clone()).unwrap();
    engine.register("proxy", proxy.clone()).unwrap();
    (engine, target, proxy)
}

fn check(engine: &mut Engine, source: &str) {
    let value = engine
        .execute(source, &mut Bare)
        .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
    assert_eq!(value, Value::Boolean(true), "{source}");
}

#[test]
fn forwarding() {
    let (mut engine, target, proxy) = setup("({x: 1})", "({})");
    check(
        &mut engine,
        "proxy.x === 1 && 'x' in proxy && Reflect.set(proxy, 'x', 2, target) && proxy.x === 2 && Reflect.deleteProperty(proxy, 'x') && !('x' in proxy);",
    );
    assert_eq!(engine.get(&target, "x").unwrap(), Value::Undefined);
    assert!(engine.get(&proxy, "x").is_err());
    assert!(engine.keys(&proxy).is_err());
    assert!(engine.prototype(&proxy).is_err());
}

#[test]
fn nesting() {
    let (mut engine, _, inner) = setup(
        "({x: 1})",
        "({get(target, key) { return target[key] + 1; }})",
    );
    let handler = engine.object(Default::default()).unwrap();
    let outer = engine.proxy(&inner, &handler).unwrap();
    engine.register("outer", outer).unwrap();
    check(
        &mut engine,
        "outer.x === 2 && Reflect.set(outer, 'x', 3) && outer.x === 4;",
    );
}

#[test]
fn describing() {
    let (mut engine, _, _) = setup("({x: 1})", "({})");
    check(
        &mut engine,
        "Reflect.set(proxy, 'x', 2) && proxy.x === 2 && Reflect.defineProperty(proxy, 'y', {value: 3, writable: true, configurable: true}) && Reflect.getOwnPropertyDescriptor(proxy, 'y').value === 3 && target.y === 3;",
    );
}

#[test]
fn defining() {
    let (mut engine, _, _) = setup(
        "({x: 1})",
        "({getOwnPropertyDescriptor(target, key) { if (key === 'x') { return {value: 9, writable: true, configurable: true}; } }, defineProperty(target, key, descriptor) { return key === 'x' && descriptor.value === 4 && !('writable' in descriptor); }})",
    );
    check(
        &mut engine,
        "Reflect.getOwnPropertyDescriptor(proxy, 'x').value === 9 && Reflect.defineProperty(proxy, 'x', {value: 4}) && target.x === 1;",
    );
}

#[test]
fn integrity() {
    let (mut engine, target, _) = setup(
        "({x: 1})",
        "({getOwnPropertyDescriptor() { return undefined; }, defineProperty() { return true; }})",
    );
    engine.prevent(&target).unwrap();
    for source in [
        "try { Reflect.getOwnPropertyDescriptor(proxy, 'x'); } catch (e) { e instanceof TypeError; }",
        "try { Reflect.defineProperty(proxy, 'y', {value: 2}); } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn fixed() {
    let (mut engine, target, _) = setup(
        "({x: 1})",
        "({getOwnPropertyDescriptor() { return {value: 2, writable: false, enumerable: true, configurable: false}; }, defineProperty() { return true; }})",
    );
    engine
        .configure(
            &target,
            &Atom::String("x".into()),
            Property {
                content: Content::Data {
                    value: Value::Number(1.0),
                    writable: false,
                },
                enumerable: true,
                configurable: false,
            },
        )
        .unwrap();
    for source in [
        "try { Reflect.getOwnPropertyDescriptor(proxy, 'x'); } catch (e) { e instanceof TypeError; }",
        "try { Reflect.defineProperty(proxy, 'x', {value: 2}); } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn prototypes() {
    let (mut engine, _, _) = setup(
        "({})",
        "({getPrototypeOf() { return target; }, setPrototypeOf() { return true; }})",
    );
    check(
        &mut engine,
        "const original = Reflect.getPrototypeOf(target); Reflect.getPrototypeOf(proxy) === target && Reflect.setPrototypeOf(proxy, null) && Reflect.getPrototypeOf(target) === original;",
    );
}

#[test]
fn lineage() {
    let (mut engine, target, _) = setup(
        "({})",
        "({getPrototypeOf() { return target; }, setPrototypeOf() { return true; }})",
    );
    engine.prevent(&target).unwrap();
    for source in [
        "try { Reflect.getPrototypeOf(proxy); false; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.setPrototypeOf(proxy, null); false; } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn integrityforward() {
    let (mut engine, _, _) = setup("({})", "({})");
    check(
        &mut engine,
        "Reflect.setPrototypeOf(proxy, null) && Reflect.getPrototypeOf(proxy) === null && Reflect.isExtensible(proxy) && Reflect.preventExtensions(proxy) && !Reflect.isExtensible(proxy);",
    );
}

#[test]
fn extensions() {
    let (mut engine, _, _) = setup(
        "({})",
        "({isExtensible(target) { return Reflect.isExtensible(target); }, preventExtensions(target) { return Reflect.preventExtensions(target); }})",
    );
    check(
        &mut engine,
        "Reflect.isExtensible(proxy) && Reflect.preventExtensions(proxy) && !Reflect.isExtensible(proxy) && !Reflect.isExtensible(target);",
    );
}

#[test]
fn constraints() {
    let (mut engine, target, _) = setup(
        "({})",
        "({getPrototypeOf() { return 1; }, setPrototypeOf() { return true; }, isExtensible() { return false; }, preventExtensions() { return true; }})",
    );
    for source in [
        "try { Reflect.getPrototypeOf(proxy); false; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.isExtensible(proxy); false; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.preventExtensions(proxy); false; } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
    engine.prevent(&target).unwrap();
    check(
        &mut engine,
        "try { Reflect.setPrototypeOf(proxy, null); false; } catch (e) { e instanceof TypeError; }",
    );
}

#[test]
fn interception() {
    let (mut engine, target, _) = setup(
        "({x: 1})",
        "({get(target, key, receiver) { if (key === 'x' && receiver === proxy) { return 7; } return 0; }, set(target, key, value) { target[key] = value + 1; return true; }, has(target, key) { return key === 'virtual'; }, deleteProperty() { return false; }})",
    );
    check(
        &mut engine,
        "proxy.x === 7 && 'virtual' in proxy && !('x' in proxy) && Reflect.set(proxy, 'x', 4) && Reflect.deleteProperty(proxy, 'x') === false;",
    );
    assert_eq!(engine.get(&target, "x").unwrap(), Value::Number(5.0));
}

#[test]
fn symbols() {
    let (mut engine, _, _) = setup(
        "({})",
        "({get(target, key) { return key; }, has(target, key) { return key === target.mark; }})",
    );
    check(
        &mut engine,
        "const key = Symbol(); target.mark = key; proxy[key] === key && key in proxy;",
    );
}

#[test]
fn invariants() {
    let (mut engine, target, _) = setup(
        "({x: 1})",
        "({get() { return 2; }, set() { return true; }, has() { return false; }, deleteProperty() { return true; }})",
    );
    engine
        .configure(
            &target,
            &Atom::String("x".into()),
            Property {
                content: Content::Data {
                    value: Value::Number(1.0),
                    writable: false,
                },
                enumerable: false,
                configurable: false,
            },
        )
        .unwrap();
    for source in [
        "try { proxy.x; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.set(proxy, 'x', 3); } catch (e) { e instanceof TypeError; }",
        "try { 'x' in proxy; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.deleteProperty(proxy, 'x'); } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn revocation() {
    let (mut engine, _, proxy) = setup("({x: 1})", "({})");
    engine.revoke(&proxy).unwrap();
    for source in [
        "try { proxy.x; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.set(proxy, 'x', 2); } catch (e) { e instanceof TypeError; }",
        "try { 'x' in proxy; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.deleteProperty(proxy, 'x'); } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn boundaries() {
    let (mut engine, target, proxy) = setup(
        "({x: 1})",
        "({has() { return false; }, deleteProperty() { return true; }})",
    );
    engine.prevent(&target).unwrap();
    for source in [
        "try { 'x' in proxy; } catch (e) { e instanceof TypeError; }",
        "try { Reflect.deleteProperty(proxy, 'x'); } catch (e) { e instanceof TypeError; }",
        "Reflect.set(proxy, 'x', 2);",
        "Reflect.ownKeys(proxy)[0] === 'x';",
        "Reflect.getOwnPropertyDescriptor(proxy, 'x').value === 2;",
    ] {
        check(&mut engine, source);
    }
    assert!(engine.properties(&proxy).is_err());
    assert!(engine.extensible(&proxy).is_err());
}
