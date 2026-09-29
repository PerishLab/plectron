mod assignment;
mod literal;
mod prototype;
mod proxy;
mod reflect;
use crate::native::Bare;
use jsengine::{Engine, Failure, Value};

#[test]
fn attributes() {
    let mut engine = Engine::default();
    let class = engine.execute("class C { constructor(value) { this.value = value; } method() {} static make() {} } C;", &mut Bare).unwrap();
    let descriptor = engine.descriptor(&class, "prototype").unwrap().unwrap();
    assert!(descriptor.writable() == Some(false));
    assert!(!descriptor.enumerable);
    assert!(!descriptor.configurable);
    let prototype = descriptor.value().unwrap();
    for (object, name) in [
        (&prototype, "constructor"),
        (&prototype, "method"),
        (&class, "make"),
    ] {
        let property = engine.descriptor(object, name).unwrap().unwrap();
        assert_eq!(property.writable(), Some(true));
        assert!(!property.enumerable);
        assert!(property.configurable);
    }
    for name in ["name", "length"] {
        let property = engine.descriptor(&class, name).unwrap().unwrap();
        assert!(property.writable() == Some(false));
        assert!(!property.enumerable);
        assert!(property.configurable);
    }
    let instance = engine
        .construct(class, vec![Value::Number(7.0)], &mut Bare)
        .unwrap();
    let property = engine.descriptor(&instance, "value").unwrap().unwrap();
    assert!(property.writable() == Some(true) && property.enumerable && property.configurable);
    assert_eq!(property.value().unwrap(), Value::Number(7.0));
    assert_eq!(engine.descriptor(&instance, "method").unwrap(), None);
    engine.set(&instance, "method", Value::Number(1.0)).unwrap();
    let property = engine.descriptor(&instance, "method").unwrap().unwrap();
    assert!(property.enumerable);
    engine
        .set(&prototype, "method", Value::Number(2.0))
        .unwrap();
    assert!(
        !engine
            .descriptor(&prototype, "method")
            .unwrap()
            .unwrap()
            .enumerable
    );
}

#[test]
fn intrinsics() {
    let mut engine = Engine::default();
    let function = engine.execute("(function () {});", &mut Bare).unwrap();
    let property = engine.descriptor(&function, "prototype").unwrap().unwrap();
    assert_eq!(property.writable(), Some(true));
    assert!(!property.enumerable);
    assert!(!property.configurable);
    let error = engine.execute("new Error('message');", &mut Bare).unwrap();
    assert!(
        !engine
            .descriptor(&error, "message")
            .unwrap()
            .unwrap()
            .enumerable
    );
    let array = engine.execute("[1, 2];", &mut Bare).unwrap();
    let property = engine.descriptor(&array, "length").unwrap().unwrap();
    assert_eq!(property.writable(), Some(true));
    assert!(!property.enumerable);
    assert!(!property.configurable);
    engine.set(&array, "length", Value::Number(1.0)).unwrap();
    assert_eq!(engine.descriptor(&array, "1").unwrap(), None);
    assert_eq!(
        engine.properties(&array).unwrap()["length"],
        Value::Number(1.0)
    );
}

#[test]
fn freezing() {
    let mut engine = Engine::default();
    let prototype = engine
        .execute(
            "const C = function () {}; C.prototype.value = 7; C.prototype;",
            &mut Bare,
        )
        .unwrap();
    engine.freeze(&prototype).unwrap();
    let property = engine.descriptor(&prototype, "value").unwrap().unwrap();
    assert!(property.writable() == Some(false));
    assert!(!property.configurable);
    assert_eq!(engine.execute("const a = new C(); try { a.value = 8; } catch (error) { error.constructor === TypeError; }", &mut Bare).unwrap(), Value::Boolean(true));
    assert_eq!(
        engine
            .execute("a.extra = 1; a.value + a.extra;", &mut Bare)
            .unwrap(),
        Value::Number(8.0)
    );
    assert!(engine.descriptor(&Value::Number(1.0), "x").is_err());
    let foreign = Engine::default().object(Default::default()).unwrap();
    assert!(engine.descriptor(&foreign, "x").is_err());
    assert!(matches!(
        engine.set(&prototype, "value", foreign),
        Err(Failure::Fault(_))
    ));
}
mod definition;
mod elements;
mod symbol;
