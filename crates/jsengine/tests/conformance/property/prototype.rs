use jsengine::{Atom, Engine, Failure, Value};

#[test]
fn rebase() {
    let mut engine = Engine::default();
    let first = engine.object(Default::default()).unwrap();
    let second = engine.object(Default::default()).unwrap();
    let child = engine.object(Default::default()).unwrap();
    engine.set(&first, "source", Value::Number(1.0)).unwrap();
    engine.set(&second, "source", Value::Number(2.0)).unwrap();
    assert!(engine.rebase(&child, &first).unwrap());
    assert_eq!(engine.get(&child, "source").unwrap(), Value::Number(1.0));
    assert!(engine.rebase(&child, &second).unwrap());
    assert_eq!(engine.get(&child, "source").unwrap(), Value::Number(2.0));
    assert!(!engine.rebase(&second, &child).unwrap());
    assert!(engine.rebase(&child, &Value::Null).unwrap());
    assert!(!engine.has(&child, &Atom::String("source".into())).unwrap());
}

#[test]
fn integrity() {
    let mut engine = Engine::default();
    let parent = engine.object(Default::default()).unwrap();
    let child = engine.object(Default::default()).unwrap();
    let original = engine.prototype(&child).unwrap();
    assert!(engine.prevent(&child).unwrap());
    assert!(engine.rebase(&child, &original).unwrap());
    assert!(!engine.rebase(&child, &parent).unwrap());
    assert_eq!(engine.prototype(&child).unwrap(), original);
    assert!(matches!(
        engine.rebase(&child, &Value::Number(1.0)),
        Err(Failure::Throw(_))
    ));
}
