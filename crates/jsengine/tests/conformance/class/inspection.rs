use crate::native::Bare;
use jsengine::{Access, Content, Engine, Failure, Value};

#[test]
fn descriptors() {
    let mut engine = Engine::default();
    let prototype = engine.execute("let calls = 0; class C { get value() { calls = calls + 1; return this; } set value(n) { calls = calls + n; } } C.prototype;", &mut Bare).unwrap();
    let property = engine.descriptor(&prototype, "value").unwrap().unwrap();
    assert!(!property.enumerable && property.configurable);
    assert_eq!(property.writable(), None);
    assert!(property.value().is_err());
    let Content::Accessor {
        get: Some(get),
        set: Some(set),
    } = property.content
    else {
        panic!("accessor pair")
    };
    for (value, name, length) in [(&get, "get value", 0.0), (&set, "set value", 1.0)] {
        assert_eq!(engine.get(value, "name").unwrap(), Value::Text(name.into()));
        assert_eq!(engine.get(value, "length").unwrap(), Value::Number(length));
        assert_eq!(engine.get(value, "prototype").unwrap(), Value::Undefined);
        assert!(matches!(
            engine.construct(value.clone(), Vec::new(), &mut Bare),
            Err(Failure::Throw(_))
        ));
    }
    assert!(engine.get(&prototype, "value").is_err());
    assert!(engine.set(&prototype, "value", Value::Number(7.0)).is_err());
    assert!(engine.properties(&prototype).is_err());
    let snapshot = engine.descriptors(&prototype).unwrap();
    assert_eq!(snapshot.len(), 2);
    assert!(matches!(
        snapshot["value"].content,
        Content::Accessor { .. }
    ));
    assert_eq!(
        engine.execute("calls;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
    let access = Access {
        base: prototype,
        receiver: Value::Number(9.0),
        key: "value".into(),
    };
    assert_eq!(engine.read(&access, &mut Bare).unwrap(), Value::Number(9.0));
    engine
        .write(&access, Value::Number(7.0), &mut Bare)
        .unwrap();
    assert_eq!(
        engine.execute("calls;", &mut Bare).unwrap(),
        Value::Number(8.0)
    );
}

#[test]
fn freezing() {
    let mut engine = Engine::default();
    let value = engine.execute("let n = 0; class C { get value() { return n; } set value(value) { n = value; } } new C();", &mut Bare).unwrap();
    let prototype = engine.prototype(&value).unwrap();
    engine.freeze(&prototype).unwrap();
    engine.freeze(&value).unwrap();
    let property = engine.descriptor(&prototype, "value").unwrap().unwrap();
    assert!(!property.configurable);
    assert_eq!(property.writable(), None);
    let access = Access::direct(&value, "value");
    engine
        .write(&access, Value::Number(7.0), &mut Bare)
        .unwrap();
    assert_eq!(engine.read(&access, &mut Bare).unwrap(), Value::Number(7.0));
    assert!(matches!(
        engine.write(&Access::direct(&value, "extra"), Value::Null, &mut Bare),
        Err(Failure::Throw(_))
    ));
}

#[test]
fn ownership() {
    let mut engine = Engine::default();
    let value = engine.execute("let calls = 0; class C { get value() { calls = calls + 1; } set value(n) { calls = calls + 1; } } new C();", &mut Bare).unwrap();
    let foreign = Engine::default().object(Default::default()).unwrap();
    let access = Access {
        base: value.clone(),
        receiver: foreign.clone(),
        key: "value".into(),
    };
    assert!(matches!(
        engine.read(&access, &mut Bare),
        Err(Failure::Fault(_))
    ));
    assert!(matches!(
        engine.write(&Access::direct(&value, "value"), foreign.clone(), &mut Bare),
        Err(Failure::Fault(_))
    ));
    assert!(matches!(
        engine.read(&Access::direct(&foreign, "value"), &mut Bare),
        Err(Failure::Fault(_))
    ));
    assert_eq!(
        engine.execute("calls;", &mut Bare).unwrap(),
        Value::Number(0.0)
    );
}

#[test]
fn attempts() {
    let mut engine = Engine::default();
    let value = engine.execute("({value: 1});", &mut Bare).unwrap();
    assert!(engine.prevent(&value).unwrap());
    assert!(
        !engine
            .attempt(
                &Access::direct(&value, "extra"),
                Value::Number(2.0),
                &mut Bare
            )
            .unwrap()
    );
    assert!(
        engine
            .attempt(
                &Access::direct(&value, "value"),
                Value::Number(3.0),
                &mut Bare
            )
            .unwrap()
    );
    assert_eq!(engine.get(&value, "value").unwrap(), Value::Number(3.0));
    assert!(matches!(
        engine.write(&Access::direct(&value, "extra"), Value::Null, &mut Bare),
        Err(Failure::Throw(_))
    ));
    engine.freeze(&value).unwrap();
    assert!(
        !engine
            .attempt(&Access::direct(&value, "value"), Value::Null, &mut Bare)
            .unwrap()
    );
}
