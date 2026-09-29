use super::check;
use jsengine::Engine;

#[test]
fn constructing() {
    let mut engine = Engine::default();
    check(
        &mut engine,
        "const proxy = new Proxy({x: 1}, {get(target, key) { return target[key] + 1; }}); proxy.x === 2;",
    );
}

#[test]
fn nesting() {
    let mut engine = Engine::default();
    check(
        &mut engine,
        "const inner = new Proxy({x: 1}, {get(target, key) { return target[key] + 1; }}); const outer = new Proxy(inner, {}); outer.x === 2;",
    );
}

#[test]
fn revoking() {
    let mut engine = Engine::default();
    check(
        &mut engine,
        "const pair = Proxy.revocable({x: 1}, {}); const before = pair.proxy.x === 1; pair.revoke(); pair.revoke(); try { pair.proxy.x; false; } catch (e) { before && e instanceof TypeError; }",
    );
}

#[test]
fn admission() {
    let mut engine = Engine::default();
    for source in [
        "try { Proxy({}, {}); false; } catch (e) { e instanceof TypeError; }",
        "try { new Proxy(1, {}); false; } catch (e) { e instanceof TypeError; }",
        "try { new Proxy({}, null); false; } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn traps() {
    let mut engine = Engine::default();
    check(
        &mut engine,
        "const proxy = new Proxy({x: 1}, {get: null, ownKeys: null}); proxy.x === 1 && Reflect.ownKeys(proxy)[0] === 'x';",
    );
    check(
        &mut engine,
        "const malformed = new Proxy({}, {get: 1}); try { malformed.x; false; } catch (e) { e instanceof TypeError; }",
    );
}
