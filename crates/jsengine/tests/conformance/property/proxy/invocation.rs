use super::{check, setup};

#[test]
fn applying() {
    let (mut engine, _, _) = setup("(function(x) { return x + 1; })", "({})");
    check(&mut engine, "Reflect.apply(proxy, null, [4]) === 5;");
}

#[test]
fn intercepting() {
    let (mut engine, _, _) = setup(
        "(function(x) { this.value = x; })",
        "({apply(target, receiver, args) { return args[0] + 2; }, construct(target, args) { return {value: args[0] + 3}; }})",
    );
    check(&mut engine, "proxy(4) === 6 && (new proxy(4)).value === 7;");
}

#[test]
fn constructing() {
    let (mut engine, _, _) = setup("(function(x) { this.value = x; })", "({})");
    check(&mut engine, "(new proxy(4)).value === 4;");
}

#[test]
fn capability() {
    let (mut engine, _, _) = setup(
        "({})",
        "({apply() { return 1; }, construct() { return {}; }})",
    );
    for source in [
        "try { proxy(); false; } catch (e) { e instanceof TypeError; }",
        "try { new proxy(); false; } catch (e) { e instanceof TypeError; }",
    ] {
        check(&mut engine, source);
    }
}

#[test]
fn result() {
    let (mut engine, _, _) = setup("(function() {})", "({construct() { return 1; }})");
    check(
        &mut engine,
        "try { new proxy(); false; } catch (e) { e instanceof TypeError; }",
    );
}
