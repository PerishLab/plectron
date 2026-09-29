use spec::harness;

#[test]
fn assertions() {
    let assertions = harness::run("assert(true); assert.sameValue(0 / 0, 0 / 0); assert.notSameValue(0, -0); assert.throws(ReferenceError, function () { missing; }); assert.throws(TypeError, function () { 1(); });").unwrap();
    assert_eq!(assertions, 5);
    for source in [
        "assert(false);",
        "assert(1);",
        "assert.sameValue(0, -0);",
        "assert.notSameValue(0 / 0, 0 / 0);",
        "assert.throws(TypeError, function () { missing; });",
        "assert.throws(Error, function () {});",
        "assert.throws(Error, function () { throw null; });",
        "assert.throws(Error, function () { throw {name: 'Error', constructor: function () {}}; });",
        "const fake = function () {}; fake.constructor = Error; assert.throws(Error, function () { throw fake; });",
    ] {
        assert!(harness::run(source).is_err(), "{source}");
    }
}

#[test]
fn constructors() {
    let source = "const a = new Test262Error('message'); assert.sameValue(a.message, 'message'); assert.sameValue(a.toString(), 'Test262Error: message'); assert(a instanceof Test262Error); assert.sameValue(a.constructor, Test262Error); const b = Test262Error('direct'); assert.sameValue(b.message, 'direct'); assert(b instanceof Test262Error); assert.notSameValue(a, b); assert.sameValue(new Test262Error().message, ''); assert.throws(Test262Error, function () { Test262Error.thrower('thrown'); });";
    assert_eq!(harness::run(source).unwrap(), 9);
    assert!(
        harness::run("assert(false);")
            .unwrap_err()
            .contains("Test262Error: assertion 1 failed")
    );
}

#[test]
fn failures() {
    for assertion in [
        "assert(false)",
        "assert()",
        "assert.throws(Error, 1)",
        "assert.throws()",
        "assert.sameValue(1, 2)",
        "assert.notSameValue(1, 1)",
    ] {
        let source = format!("assert.throws(Test262Error, function () {{ {assertion}; }});");
        assert_eq!(harness::run(&source).unwrap(), 2, "{source}");
    }
    assert_eq!(harness::run("assert.sameValue(); assert.sameValue(undefined); try { assert(false); } catch (error) { assert(error instanceof Test262Error); }").unwrap(), 4);
}

#[test]
fn termination() {
    let source = "assert.throws(Error, function () { const recurse = function () { return recurse(); }; recurse(); });";
    let error = harness::run(source).unwrap_err();
    assert!(error.contains("evaluation depth exhausted"));
}

#[test]
fn accessors() {
    assert_eq!(harness::run("let calls = 0; const value = {get constructor() { calls = calls + 1; return TypeError; }}; assert.throws(TypeError, function () { throw value; }); assert.sameValue(calls, 1);").unwrap(), 2);
    assert_eq!(harness::run("const value = {get constructor() { throw 7; }}; try { assert.throws(TypeError, function () { throw value; }); } catch (error) { assert.sameValue(error, 7); }").unwrap(), 2);
    let error =
        harness::run("throw {get name() { throw 1; }, get message() { throw 2; }};").unwrap_err();
    assert!(error.contains("uncaught exception: [object Object]"));
}
