use crate::native::Bare;
use jsengine::{Engine, Value};

#[test]
fn operations() {
    for source in [
        "const o = {x: 1}; Reflect.get(o, 'x') === 1 && Reflect.set(o, 'x', 2) && Reflect.has(o, 'x') && Reflect.get(o, 'x') === 2 && Reflect.deleteProperty(o, 'x') && !Reflect.has(o, 'x');",
        "const key = Symbol(); const o = {[key]: 7}; Reflect.get(o, key) === 7 && Reflect.has(o, key) && Reflect.deleteProperty(o, key) && !Reflect.has(o, key);",
        "const o = {get x() { return this.value; }, set x(v) { this.value = v; }}; const receiver = {value: 4}; Reflect.get(o, 'x', receiver) === 4 && Reflect.set(o, 'x', 8, receiver) && receiver.value === 8;",
        "const base = {get x() { return this.value; }, set x(v) { this.value = v; }}; const child = {}; const receiver = {value: 4}; Reflect.setPrototypeOf(child, base) && Reflect.has(child, 'x') && Reflect.get(child, 'x', receiver) === 4 && Reflect.set(child, 'x', 9, receiver) && receiver.value === 9;",
        "const o = {x: 1}; Reflect.isExtensible(o) && Reflect.preventExtensions(o) && !Reflect.isExtensible(o) && !Reflect.set(o, 'y', 2) && Reflect.set(o, 'x', 3) && o.x === 3;",
        "const a = {}; const b = {}; Reflect.setPrototypeOf(a, b) && Reflect.getPrototypeOf(a) === b && !Reflect.setPrototypeOf(b, a) && Reflect.preventExtensions(a) && Reflect.setPrototypeOf(a, b) && !Reflect.setPrototypeOf(a, null);",
        "const key = Symbol(); const o = {later: 1, 2: 2, [key]: 3, 0: 4}; const keys = Reflect.ownKeys(o); keys.length === 4 && keys[0] === '0' && keys[1] === '2' && keys[2] === 'later' && keys[3] === key;",
        "let caught = false; try { Reflect.get(1, 'x'); } catch (e) { caught = e instanceof TypeError; } caught;",
    ] {
        let mut engine = Engine::default();
        let result = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
}

#[test]
fn descriptors() {
    for source in [
        "const o = {}; Reflect.defineProperty(o, 'x', {value: 3}) && o.x === 3 && Reflect.getOwnPropertyDescriptor(o, 'x').writable === false && !Reflect.defineProperty(o, 'x', {value: 4});",
        "const o = {x: 1}; Reflect.defineProperty(o, 'x', {writable: false, configurable: false}) && Reflect.getOwnPropertyDescriptor(o, 'x').value === 1 && !Reflect.set(o, 'x', 2);",
        "const o = {}; Reflect.getOwnPropertyDescriptor(o, 'missing') === undefined;",
        "const o = {}; let caught = false; try { Reflect.defineProperty(o, 'x', {value: 1, get() { return 2; }}); } catch (e) { caught = e instanceof TypeError; } caught;",
        "const o = {}; let caught = false; try { Reflect.defineProperty(o, 'x', {get: 3}); } catch (e) { caught = e instanceof TypeError; } caught;",
        "const o = {}; let calls = 0; const d = {get value() { calls = calls + 1; return 5; }}; Reflect.defineProperty(o, 'x', d) && o.x === 5 && calls === 1;",
        "const o = {x: 1}; Reflect.defineProperty(o, 'x', {get() { return 2; }}) && Reflect.getOwnPropertyDescriptor(o, 'x').enumerable && Reflect.getOwnPropertyDescriptor(o, 'x').configurable && o.x === 2;",
    ] {
        let mut engine = Engine::default();
        let result = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
}

#[test]
fn invocation() {
    for source in [
        "const f = function(a, b) { return this.x + a + b; }; Reflect.apply(f, {x: 3}, [4, 5]) === 12;",
        "const C = function(value) { this.value = value; }; const o = Reflect.construct(C, [7]); o.value === 7 && o instanceof C;",
        "const C = function() {}; const D = function() {}; const o = Reflect.construct(C, [], D); Reflect.getPrototypeOf(o) === D.prototype;",
        "let caught = false; try { Reflect.apply({}, null, []); } catch (e) { caught = e instanceof TypeError; } caught;",
        "let calls = 0; const args = {get length() { calls = calls + 1; return 0; }}; try { Reflect.apply({}, null, args); } catch (e) {} calls === 0;",
        "const C = function() {}; let calls = 0; const args = {get length() { calls = calls + 1; return 0; }}; try { Reflect.construct(C, args, {}); } catch (e) {} calls === 0;",
    ] {
        let mut engine = Engine::default();
        let result = engine
            .execute(source, &mut Bare)
            .unwrap_or_else(|failure| panic!("{}: {source}", engine.diagnostic(&failure)));
        assert_eq!(result, Value::Boolean(true), "{source}");
    }
}
