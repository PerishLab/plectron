use crate::native::Bare;
use jsengine::{Engine, Value};

#[test]
fn storage() {
    let mut engine = Engine::default();
    let array = engine.array(vec![None, Some(Value::Number(2.0))]).unwrap();
    let parent = engine.prototype(&array).unwrap();
    engine.set(&parent, "0", Value::Number(9.0)).unwrap();
    assert_eq!(engine.get(&array, "0").unwrap(), Value::Number(9.0));
    assert_eq!(engine.descriptor(&array, "0").unwrap(), None);
    for (key, value) in [
        ("0", 1.0),
        ("4", 5.0),
        ("2", 3.0),
        ("3", 4.0),
        ("4", 6.0),
        ("01", 7.0),
        ("+1", 8.0),
    ] {
        engine.set(&array, key, Value::Number(value)).unwrap();
    }
    assert_eq!(engine.get(&array, "length").unwrap(), Value::Number(5.0));
    let values = engine.properties(&array).unwrap();
    let descriptors = engine.descriptors(&array).unwrap();
    assert_eq!(values.len(), 8);
    for (name, value) in values {
        assert_eq!(descriptors[&name].value().unwrap(), value);
    }
    engine.set(&array, "length", Value::Number(1.0)).unwrap();
    engine.set(&array, "length", Value::Number(5.0)).unwrap();
    assert_eq!(engine.get(&array, "4").unwrap(), Value::Undefined);
    assert_eq!(engine.get(&array, "01").unwrap(), Value::Number(7.0));
    engine.freeze(&array).unwrap();
    assert!(engine.set(&array, "0", Value::Null).is_err());
    assert!(engine.set(&array, "4", Value::Null).is_err());
    assert_eq!(
        engine.descriptor(&array, "0").unwrap().unwrap().writable(),
        Some(false)
    );
}
#[test]
fn budget() {
    let mut engine = Engine::default();
    let array = engine.array(vec![Some(Value::Null); 4095]).unwrap();
    engine
        .set(&array, "4294967294", Value::Number(1.0))
        .unwrap();
    assert!(engine.set(&array, "extra", Value::Null).is_err());
    engine.set(&array, "0", Value::Number(2.0)).unwrap();
    engine.set(&array, "length", Value::Number(1.0)).unwrap();
    engine.set(&array, "extra", Value::Number(3.0)).unwrap();
    assert_eq!(engine.properties(&array).unwrap().len(), 3);
    let foreign = Engine::default().array(vec![]).unwrap();
    assert!(engine.array(vec![Some(foreign)]).is_err());
}
#[test]
fn iteration() {
    let mut engine = Engine::default();
    let value = engine.execute("const a = [1,,3]; let sum = 0; for (const x of a) { if (x === 1) { a[1] = 2; a[a.length] = 4; } sum = sum + x; } sum;", &mut Bare).unwrap();
    assert_eq!(value, Value::Number(10.0));
    assert_eq!(engine.execute("let copies = []; for (const x of a) { copies[copies.length] = () => x; } copies[0]() + copies[3]();", &mut Bare).unwrap(), Value::Number(5.0));
}
