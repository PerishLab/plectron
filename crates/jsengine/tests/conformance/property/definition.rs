use crate::native::Bare;
use jsengine::{Access, Atom, Content, Engine, Failure, Property, Value};

fn data(value: Value, writable: bool, configurable: bool) -> Property {
    Property {
        content: Content::Data { value, writable },
        enumerable: true,
        configurable,
    }
}

#[test]
fn fixed() {
    let mut engine = Engine::default();
    let object = engine.object(Default::default()).unwrap();
    let key = Atom::String("value".into());
    let property = data(Value::Number(1.0), false, false);
    assert!(engine.configure(&object, &key, property.clone()).unwrap());
    assert!(engine.configure(&object, &key, property.clone()).unwrap());
    for candidate in [
        data(Value::Number(2.0), false, false),
        data(Value::Number(1.0), true, false),
        data(Value::Number(1.0), false, true),
        Property {
            enumerable: false,
            ..property.clone()
        },
        Property {
            content: Content::Accessor {
                get: None,
                set: None,
            },
            ..property.clone()
        },
    ] {
        assert!(!engine.configure(&object, &key, candidate).unwrap());
    }
    assert_eq!(engine.descriptor(&object, "value").unwrap(), Some(property));
}

#[test]
fn transitions() {
    let mut engine = Engine::default();
    let object = engine.object(Default::default()).unwrap();
    let key = Atom::String("value".into());
    assert!(
        engine
            .configure(&object, &key, data(Value::Number(1.0), true, false))
            .unwrap()
    );
    assert!(
        engine
            .configure(&object, &key, data(Value::Number(2.0), true, false))
            .unwrap()
    );
    assert!(
        engine
            .configure(&object, &key, data(Value::Number(3.0), false, false))
            .unwrap()
    );
    assert!(
        !engine
            .configure(&object, &key, data(Value::Number(4.0), false, false))
            .unwrap()
    );
    assert!(engine.prevent(&object).unwrap());
    assert!(
        !engine
            .configure(
                &object,
                &Atom::String("new".into()),
                data(Value::Null, true, true),
            )
            .unwrap()
    );
    assert_eq!(engine.get(&object, "value").unwrap(), Value::Number(3.0));
}

#[test]
fn identities() {
    let mut engine = Engine::default();
    let object = engine.object(Default::default()).unwrap();
    let key = Atom::String("number".into());
    assert!(
        engine
            .configure(&object, &key, data(Value::Number(f64::NAN), false, false))
            .unwrap()
    );
    assert!(
        engine
            .configure(&object, &key, data(Value::Number(f64::NAN), false, false))
            .unwrap()
    );
    let zero = Atom::String("zero".into());
    assert!(
        engine
            .configure(&object, &zero, data(Value::Number(0.0), false, false))
            .unwrap()
    );
    assert!(
        !engine
            .configure(&object, &zero, data(Value::Number(-0.0), false, false))
            .unwrap()
    );
    let foreign = Engine::default().execute("Symbol();", &mut Bare).unwrap();
    let Value::Symbol(symbol) = foreign else {
        unreachable!()
    };
    assert!(matches!(
        engine.configure(
            &object,
            &Atom::Symbol(symbol),
            data(Value::Null, true, true)
        ),
        Err(Failure::Fault(_))
    ));
}

#[test]
fn arrays() {
    let mut engine = Engine::default();
    let array = engine.execute("[1, 2, 3];", &mut Bare).unwrap();
    let key = Atom::String("length".into());
    let length = Property {
        content: Content::Data {
            value: Value::Number(1.0),
            writable: true,
        },
        enumerable: false,
        configurable: false,
    };
    assert!(engine.configure(&array, &key, length).unwrap());
    assert_eq!(engine.get(&array, "length").unwrap(), Value::Number(1.0));
    assert_eq!(engine.descriptor(&array, "2").unwrap(), None);
    let invalid = Property {
        content: Content::Data {
            value: Value::Number(1.5),
            writable: true,
        },
        enumerable: false,
        configurable: false,
    };
    assert!(matches!(
        engine.configure(&array, &key, invalid),
        Err(Failure::Throw(_))
    ));
}

#[test]
fn accessors() {
    let mut engine = Engine::default();
    let object = engine.object(Default::default()).unwrap();
    let key = Atom::String("value".into());
    assert!(
        engine
            .configure(&object, &key, data(Value::Number(1.0), true, true))
            .unwrap()
    );
    let getter = engine
        .execute("(function () { return 7; });", &mut Bare)
        .unwrap();
    let property = Property {
        content: Content::Accessor {
            get: Some(getter),
            set: None,
        },
        enumerable: true,
        configurable: false,
    };
    assert!(engine.configure(&object, &key, property.clone()).unwrap());
    assert_eq!(
        engine
            .read(&Access::direct(&object, "value"), &mut Bare)
            .unwrap(),
        Value::Number(7.0)
    );
    assert!(engine.configure(&object, &key, property.clone()).unwrap());
    let changed = Property {
        content: Content::Accessor {
            get: None,
            set: None,
        },
        ..property
    };
    assert!(!engine.configure(&object, &key, changed).unwrap());
}
