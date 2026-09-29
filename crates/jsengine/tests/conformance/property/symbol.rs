use crate::native::Bare;
use jsengine::{Atom, Engine, Value};

#[test]
fn identity() {
    for source in [
        "const a = Symbol('same'); const b = Symbol('same'); a !== b && a === a;",
        "typeof Symbol.plectron === 'symbol' && Symbol.plectron === Symbol.plectron && Symbol.plectron !== Symbol('plectron') && Symbol.plectron !== Symbol.iterator;",
        "const value = {plectron: 1, [Symbol.plectron]: 2}; value.plectron === 1 && value['plectron'] === 1 && value[Symbol.plectron] === 2;",
        "const key = Symbol('id'); const value = {[key]: 7, id: 9}; value[key] === 7 && value.id === 9;",
        "const key = Symbol(); const value = {}; value[key] = 4; value[key] === 4 && value.undefined === undefined;",
        "const key = Symbol(); const value = [3]; value[key] = 8; value[key] === 8 && value.length === 1 && value[0] === 3;",
        "const key = Symbol(); const value = {[key]: 1}; value[key] += 2; value[key] === 3;",
        "const key = Symbol(); const value = {[key]() { return this; }}; value[key]() === value;",
        "const key = Symbol(); class Example { [key]() { return 12; } } new Example()[key]() === 12;",
        "const key = Symbol(); let add = false; let number = false; try { key + ''; } catch (error) { add = error instanceof TypeError; } try { +key; } catch (error) { number = error instanceof TypeError; } add && number && !!key;",
    ] {
        let mut engine = Engine::default();
        let result = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
}

#[test]
fn intrinsics() {
    for name in ["iterator", "plectron"] {
        let mut engine = Engine::default();
        let source = format!(
            "const descriptor = Reflect.getOwnPropertyDescriptor(Symbol, '{name}'); descriptor.value === Symbol.{name} && descriptor.writable === false && descriptor.enumerable === false && descriptor.configurable === false;"
        );
        let result = engine
            .execute(&source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
}

#[test]
fn reflection() {
    for source in [
        "const value = {[Symbol.plectron]: 7}; Reflect.get(value, Symbol.plectron) === 7 && Reflect.ownKeys(value)[0] === Symbol.plectron;",
        "let seen = undefined; const target = {[Symbol.plectron]: 4}; const proxy = new Proxy(target, {get(value, key, receiver) { seen = key; return Reflect.get(value, key, receiver); }}); proxy[Symbol.plectron] === 4 && seen === Symbol.plectron;",
    ] {
        let mut engine = Engine::default();
        let result = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
}

#[test]
fn ordering() {
    let mut engine = Engine::default();
    let symbol = engine.execute("Symbol('first');", &mut Bare).unwrap();
    engine.register("first", symbol.clone()).unwrap();
    let object = engine
        .execute(
            "const value = {later: 1, 2: 2, [first]: 3, earlier: 4, 0: 5}; value;",
            &mut Bare,
        )
        .unwrap();
    assert_eq!(
        engine.keys(&object).unwrap(),
        vec![
            Atom::String("0".into()),
            Atom::String("2".into()),
            Atom::String("later".into()),
            Atom::String("earlier".into()),
            match symbol {
                Value::Symbol(symbol) => Atom::Symbol(symbol),
                _ => unreachable!(),
            },
        ]
    );
}

#[test]
fn ownership() {
    let mut source = Engine::default();
    let symbol = source
        .execute("Symbol();", &mut Bare)
        .unwrap_or_else(|failure| panic!("{}", source.diagnostic(&failure)));
    let mut other = Engine::default();
    assert!(
        other
            .object(std::collections::BTreeMap::from([("key".into(), symbol)]))
            .is_err()
    );
}

#[test]
fn integrity() {
    let mut engine = Engine::default();
    let symbol = engine.execute("Symbol();", &mut Bare).unwrap();
    engine.register("key", symbol.clone()).unwrap();
    let object = engine
        .execute("({plain: 1, [key]: 2});", &mut Bare)
        .unwrap();
    let Value::Symbol(symbol) = symbol else {
        unreachable!()
    };
    let key = Atom::Symbol(symbol);
    assert!(engine.has(&object, &key).unwrap());
    assert!(engine.prevent(&object).unwrap());
    assert!(!engine.extensible(&object).unwrap());
    assert!(engine.set(&object, "added", Value::Number(3.0)).is_err());
    engine.set(&object, "plain", Value::Number(4.0)).unwrap();
    assert!(engine.delete(&object, &key).unwrap());
    assert!(!engine.has(&object, &key).unwrap());
    let array = engine.execute("[1];", &mut Bare).unwrap();
    assert!(
        !engine
            .delete(&array, &Atom::String("length".into()))
            .unwrap()
    );
}

#[test]
fn operators() {
    for source in [
        "const key = Symbol(); const value = {[key]: 1}; key in value && delete value[key] && !(key in value);",
        "const value = {x: 1}; 'x' in value && delete value.x && !('x' in value);",
        "class Base { get x() { return 1; } } const value = new Base(); 'x' in value && delete value.x && 'x' in value;",
        "let calls = 0; const value = {get x() { calls = calls + 1; }}; 'x' in value && calls === 0;",
        "const value = [1, 2]; delete value[0] && !(0 in value) && value.length === 2 && 1 in value;",
        "delete ({x: 1}).missing && delete 7;",
        "delete 'abc'.slice && delete 'abc'.missing;",
        "let caught = false; try { 'x' in 1; } catch (error) { caught = error instanceof TypeError; } caught;",
        "let caught = false; try { delete [1].length; } catch (error) { caught = error instanceof TypeError; } caught;",
        "let caught = false; try { delete 'abc'[0]; } catch (error) { caught = error instanceof TypeError; } caught;",
    ] {
        let mut engine = Engine::default();
        let result = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
    for source in ["delete value;", "delete (value);"] {
        assert!(Engine::check(source).is_err(), "{source}");
    }
}
