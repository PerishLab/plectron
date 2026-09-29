use jsengine::{Addons, Engine, Failure, Invocation, Value};

struct Host;
impl Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}

#[test]
fn identity() {
    let mut engine = Engine::default();
    let object = engine.object(Default::default()).unwrap();
    let first = engine.retain(&object).unwrap();
    assert_eq!(engine.held(first).unwrap(), object);
    engine.release(first).unwrap();
    assert!(engine.held(first).is_err());
    assert!(engine.release(first).is_err());

    let second = engine.retain(&Value::Number(7.0)).unwrap();
    assert_ne!(first, second);
    assert!(engine.held(first).is_err());
    assert_eq!(engine.held(second).unwrap(), Value::Number(7.0));

    let mut other = Engine::default();
    assert!(other.held(second).is_err());
    assert!(other.release(second).is_err());
    assert!(other.retain(&object).is_err());
}

#[test]
fn reachability() {
    let mut engine = Engine::default();
    let object = engine
        .execute("let cycle = {}; cycle.self = cycle; cycle;", &mut Host)
        .unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&object));
    engine.execute("cycle = null;", &mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&object));

    let closure = engine
        .execute(
            "let saved = (() => { const value = {}; return () => value; })(); saved;",
            &mut Host,
        )
        .unwrap();
    let captured = engine.call(closure.clone(), vec![], &mut Host).unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&captured));
    engine.execute("saved = null;", &mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&closure));
    assert!(!engine.reachability(&Host).unwrap().contains(&captured));
}

#[test]
fn asynchronous() {
    let mut engine = Engine::default();
    let object = engine
        .execute(
            "let object = {}; let promise = Promise.resolve(object); object;",
            &mut Host,
        )
        .unwrap();
    engine.execute("object = null;", &mut Host).unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&object));
    engine.execute("promise = null;", &mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&object));

    let queued = engine
        .execute(
            "let held = (() => { const value = {}; Promise.resolve(0).then(() => value); return value; })(); held;",
            &mut Host,
        )
        .unwrap();
    engine.execute("held = null;", &mut Host).unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&queued));
    engine.checkpoint(&mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&queued));

    let received = engine
        .execute(
            "let received = {}; Promise.resolve(received).then(() => 1); received;",
            &mut Host,
        )
        .unwrap();
    engine.execute("received = null;", &mut Host).unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&received));
    engine.checkpoint(&mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&received));
}

#[test]
fn suspension() {
    let mut engine = Engine::default();
    let object = engine
        .execute(
            "let resolve = null; let held = (() => { const value = {}; const gate = new Promise(done => { resolve = done; }); const run = async () => { await gate; return value; }; run(); return value; })(); held;",
            &mut Host,
        )
        .unwrap_or_else(|failure| panic!("{}", engine.diagnostic(&failure)));
    engine.execute("held = null;", &mut Host).unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&object));
    engine.execute("resolve = null;", &mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&object));
}

#[test]
fn collection() {
    let mut engine = Engine::default();
    engine.collect(&mut Host).unwrap();
    let old = engine.object(Default::default()).unwrap();
    let Value::Object(previous) = old else {
        unreachable!()
    };
    assert_eq!(engine.collect(&mut Host).unwrap(), 1);
    assert!(engine.retain(&old).is_err());
    let fresh = engine.object(Default::default()).unwrap();
    let Value::Object(current) = fresh else {
        unreachable!()
    };
    assert_eq!(previous.id(), current.id());
    assert_ne!(old, fresh);
    assert!(engine.properties(&old).is_err());

    let root = engine.retain(&fresh).unwrap();
    engine.collect(&mut Host).unwrap();
    assert_eq!(engine.held(root).unwrap(), fresh);
    engine.release(root).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&fresh).is_err());
}

#[test]
fn symbolic() {
    let mut engine = Engine::default();
    let target = engine
        .execute(
            "let holder = {}; let target = {}; holder[Symbol.iterator] = target; target;",
            &mut Host,
        )
        .unwrap();
    engine.execute("target = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&target).is_ok());
    engine.execute("holder = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&target).is_err());
}

#[test]
fn cycles() {
    let mut engine = Engine::default();
    engine.collect(&mut Host).unwrap();
    let cycle = engine
        .execute("let cycle = {}; cycle.self = cycle; cycle;", &mut Host)
        .unwrap();
    engine.execute("cycle = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&cycle).is_err());

    let suspended = engine
        .execute(
            "let resolve = null; let held = (() => { const value = {}; const gate = new Promise(done => { resolve = done; }); const run = async () => { await gate; return value; }; run(); return value; })(); held;",
            &mut Host,
        )
        .unwrap();
    engine.execute("held = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&suspended).is_ok());
    engine.execute("resolve = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&suspended).is_err());
}

#[test]
fn promises() {
    let mut engine = Engine::default();
    let object = engine
        .execute(
            "let object = {}; let promise = Promise.resolve(object); object;",
            &mut Host,
        )
        .unwrap();
    engine.execute("object = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&object).is_ok());
    engine.execute("promise = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    assert!(engine.properties(&object).is_err());
}

#[test]
fn reuse() {
    let mut engine = Engine::default();
    engine.collect(&mut Host).unwrap();
    let mut first = None;
    for _ in 0..32 {
        let mut slots = Vec::new();
        for _ in 0..16 {
            let Value::Object(handle) = engine.object(Default::default()).unwrap() else {
                unreachable!()
            };
            slots.push(handle.id());
        }
        slots.sort_unstable();
        if let Some(expected) = &first {
            assert_eq!(&slots, expected);
        } else {
            first = Some(slots);
        }
        assert_eq!(engine.collect(&mut Host).unwrap(), 16);
    }
}

#[test]
fn capacity() {
    let mut engine = Engine::default();
    engine.collect(&mut Host).unwrap();
    let mut previous = None;
    for _ in 0..24 {
        engine
            .execute(
                "(() => { const value = {}; const closure = () => value; new Promise(resolve => resolve(closure)); })();",
                &mut Host,
            )
            .unwrap();
        engine.collect(&mut Host).unwrap();
        let slots = engine.slots();
        if let Some(previous) = previous {
            assert_eq!(slots, previous);
        }
        previous = Some(slots);
    }
}
