use jsengine::{Addons, Engine, Failure, Invocation, Value};

struct Host;
impl Addons for Host {
    fn call(&mut self, _: &mut Engine, _: usize, _: Invocation) -> Result<Value, Failure> {
        unreachable!()
    }
}

#[test]
fn ephemerons() {
    let mut engine = Engine::default();
    let key = engine
        .execute(
            "let map = new WeakMap(); let key = {}; let value = {key: key}; map.set(key, value); key;",
            &mut Host,
        )
        .unwrap();
    let value = engine.execute("value;", &mut Host).unwrap();
    engine.execute("value = null;", &mut Host).unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&value));
    engine.execute("key = null;", &mut Host).unwrap();
    let reachable = engine.reachability(&Host).unwrap();
    assert!(!reachable.contains(&key));
    assert!(!reachable.contains(&value));
    engine.collect(&mut Host).unwrap();
    assert!(engine.retain(&key).is_err());
    assert!(engine.retain(&value).is_err());
}

#[test]
fn chains() {
    let mut engine = Engine::default();
    let key = engine
        .execute("let first = new WeakMap(); let second = new WeakMap(); let key = {}; let middle = {}; let value = {}; first.set(key, middle); second.set(middle, value); middle;", &mut Host)
        .unwrap();
    let value = engine.execute("value;", &mut Host).unwrap();
    engine
        .execute("middle = null; value = null;", &mut Host)
        .unwrap();
    assert!(engine.reachability(&Host).unwrap().contains(&key));
    assert!(engine.reachability(&Host).unwrap().contains(&value));
    engine.execute("key = null;", &mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&key));
    assert!(!engine.reachability(&Host).unwrap().contains(&value));
}

#[test]
fn members() {
    let mut engine = Engine::default();
    let key = engine
        .execute(
            "let set = new WeakSet(); let key = {}; set.add(key); key;",
            &mut Host,
        )
        .unwrap();
    engine.execute("key = null;", &mut Host).unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&key));
    engine.collect(&mut Host).unwrap();
    assert!(engine.retain(&key).is_err());
}

#[test]
fn expiration() {
    let mut engine = Engine::default();
    let old = engine
        .execute(
            "let map = new WeakMap(); let key = {}; map.set(key, 1); key;",
            &mut Host,
        )
        .unwrap();
    engine.execute("key = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    let fresh = engine.execute("let fresh = {}; fresh;", &mut Host).unwrap();
    assert_ne!(old, fresh);
    assert_eq!(
        engine.execute("map.has(fresh);", &mut Host).unwrap(),
        Value::Boolean(false)
    );
    let value = engine
        .execute("let value = {}; map.set(fresh, value); value;", &mut Host)
        .unwrap();
    engine
        .execute("map = null; value = null;", &mut Host)
        .unwrap();
    assert!(!engine.reachability(&Host).unwrap().contains(&value));
}

#[test]
fn reporting() {
    let mut engine = Engine::default();
    engine.collect(&mut Host).unwrap();
    engine
        .execute(
            "let first = new WeakMap(); let second = new WeakMap(); let key = {}; let middle = {}; let value = {}; first.set(key, middle); second.set(middle, value); middle = null; value = null;",
            &mut Host,
        )
        .unwrap();
    engine.collect(&mut Host).unwrap();
    let retained = engine.collection();
    assert_eq!(retained.weak, 2);
    assert_eq!(retained.edges, 2);
    assert_eq!(retained.activations, 2);
    assert_eq!(retained.rounds, 3);
    assert_eq!(retained.live + retained.vacant, retained.capacity);

    engine.execute("key = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    let released = engine.collection();
    assert_eq!(released.weak, 2);
    assert_eq!(released.edges, 0);
    assert_eq!(released.activations, 0);
    assert_eq!(released.rounds, 1);
    assert!(released.released >= 3);
    assert_eq!(released.live + released.vacant, released.capacity);
}

#[test]
fn bounded() {
    let mut engine = Engine::default();
    engine.collect(&mut Host).unwrap();
    let source = format!(
        "(() => {{ {} }})();",
        (0..128)
            .map(|index| format!("const key{index} = {{}}; const value{index} = {{}}; const map{index} = new WeakMap(); map{index}.set(key{index}, value{index});"))
            .collect::<String>()
    );
    let mut capacity = None;
    for _ in 0..8 {
        engine.execute(&source, &mut Host).unwrap();
        engine.collect(&mut Host).unwrap();
        let report = engine.collection();
        assert_eq!(report.weak, 0);
        assert_eq!(report.edges, 0);
        assert_eq!(report.live + report.vacant, report.capacity);
        if let Some(capacity) = capacity {
            assert_eq!(report.capacity, capacity);
        } else {
            capacity = Some(report.capacity);
        }
    }
}

#[test]
fn propagation() {
    let mut engine = Engine::default();
    let depth = 128;
    let mut source = String::from("let chain0 = {}; ");
    for index in 0..depth {
        source.push_str(&format!(
            "let chain{} = {{}}; let weak{index} = new WeakMap(); weak{index}.set(chain{index}, chain{}); ",
            index + 1,
            index + 1,
        ));
    }
    for index in 1..=depth {
        source.push_str(&format!("chain{index} = null; "));
    }
    engine.execute(&source, &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    let retained = engine.collection();
    assert_eq!(retained.weak, depth);
    assert_eq!(retained.edges, depth);
    assert_eq!(retained.activations, depth);
    assert_eq!(retained.rounds, depth + 1);

    engine.execute("chain0 = null;", &mut Host).unwrap();
    engine.collect(&mut Host).unwrap();
    let released = engine.collection();
    assert_eq!(released.weak, depth);
    assert_eq!(released.edges, 0);
    assert_eq!(released.activations, 0);
    assert_eq!(released.rounds, 1);
    assert!(released.released > depth);
}
