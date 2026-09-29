use domengine::{Document, Sheet, style::animation::Animations};
use jsengine::Value;
use jsruntime::Runtime;
use std::sync::{Arc, Mutex};
use webengine::Web;

fn runtime() -> Runtime<Web> {
    let caches = shared::cache::Manager::default();
    let web = Web::new(
        Document::default(),
        (
            Sheet::default(),
            Arc::new(Mutex::new(Animations::default())),
            std::time::Instant::now(),
        ),
        config::Store::default(),
        &caches,
    )
    .unwrap();
    let mut runtime = Runtime::new(web).unwrap();
    runtime
        .execute(&fixtures::script("document, events, bridge, scroll", ""))
        .unwrap();
    runtime
}

fn live(runtime: &mut Runtime<Web>, value: &Value) -> bool {
    runtime
        .host(|engine, web| Ok(engine.reachability(web)?.contains(value)))
        .unwrap()
}

#[test]
fn detached() {
    let mut runtime = runtime();
    let pair = runtime
        .execute("let pair = (() => { const node = document.createElement('div'); const callback = () => node; node.addEventListener('test', callback); return [node, callback]; })(); pair;")
        .unwrap();
    let (wrapper, callback) = runtime
        .host(|engine, _| Ok((engine.get(&pair, "0")?, engine.get(&pair, "1")?)))
        .unwrap();
    assert!(live(&mut runtime, &wrapper));
    assert!(live(&mut runtime, &callback));
    runtime.execute("pair = null;").unwrap();
    assert!(!live(&mut runtime, &wrapper));
    assert!(!live(&mut runtime, &callback));

    let root = runtime.retain(&wrapper).unwrap();
    assert_eq!(runtime.held(root).unwrap(), wrapper);
    assert!(live(&mut runtime, &wrapper));
    assert!(live(&mut runtime, &callback));
    runtime.release(root).unwrap();
    assert!(runtime.held(root).is_err());
    assert!(!live(&mut runtime, &wrapper));
    assert!(!live(&mut runtime, &callback));
}

#[test]
fn component() {
    let mut runtime = runtime();
    let pair = runtime
        .execute("let pair = (() => { const parent = document.createElement('div'); const child = document.createElement('span'); const callback = () => child; child.addEventListener('test', callback); parent.appendChild(child); return [parent, child, callback]; })(); pair;")
        .unwrap();
    let (parent, child, callback) = runtime
        .host(|engine, _| {
            Ok((
                engine.get(&pair, "0")?,
                engine.get(&pair, "1")?,
                engine.get(&pair, "2")?,
            ))
        })
        .unwrap();
    runtime.execute("pair = null;").unwrap();
    let root = runtime.retain(&parent).unwrap();
    assert!(live(&mut runtime, &child));
    assert!(live(&mut runtime, &callback));
    runtime.release(root).unwrap();
    assert!(!live(&mut runtime, &parent));
    assert!(!live(&mut runtime, &child));
    assert!(!live(&mut runtime, &callback));
}

#[test]
fn listener() {
    let mut runtime = runtime();
    let callback = runtime.execute("let scope = events.scope(); let node = document.createElement('div'); document.root.appendChild(node); let callback = () => 1; node.addEventListener('test', callback, {scope: scope}); callback;").unwrap();
    runtime.execute("callback = null;").unwrap();
    assert!(live(&mut runtime, &callback));
    runtime.execute("scope.dispose();").unwrap();
    assert!(!live(&mut runtime, &callback));
}

#[test]
fn receiver() {
    let mut runtime = runtime();
    let callback = runtime
        .execute("let callback = () => 1; bridge.onMessage(callback); callback;")
        .unwrap();
    runtime.execute("callback = null;").unwrap();
    assert!(live(&mut runtime, &callback));
    runtime.execute("bridge.onMessage(() => 2);").unwrap();
    assert!(!live(&mut runtime, &callback));
}

#[test]
fn native() {
    let mut runtime = runtime();
    let pair = runtime.execute("let pair = (() => { const node = document.createElement('div'); const callback = () => node; node.addEventListener('test', callback); return [node.style.getPropertyValue, node, callback]; })(); pair;").unwrap();
    let (method, wrapper, callback) = runtime
        .host(|engine, _| {
            Ok((
                engine.get(&pair, "0")?,
                engine.get(&pair, "1")?,
                engine.get(&pair, "2")?,
            ))
        })
        .unwrap();
    runtime.execute("pair = null;").unwrap();
    assert!(!live(&mut runtime, &wrapper));
    let root = runtime.retain(&method).unwrap();
    assert!(live(&mut runtime, &wrapper));
    assert!(live(&mut runtime, &callback));
    runtime.release(root).unwrap();
    assert!(!live(&mut runtime, &wrapper));
    assert!(!live(&mut runtime, &callback));
}

#[test]
fn collection() {
    let mut runtime = runtime();
    let wrapper = runtime
        .execute("let node = document.createElement('div'); node.addEventListener('test', () => node); node;")
        .unwrap();
    runtime.execute("node = null;").unwrap();
    runtime.collect().unwrap();
    assert!(runtime.retain(&wrapper).is_err());
    let fresh = runtime.execute("document.createElement('div');").unwrap();
    assert_ne!(wrapper, fresh);
    assert!(runtime.retain(&fresh).is_ok());

    let connected = runtime.execute("let connected = document.createElement('div'); document.root.appendChild(connected); connected;").unwrap();
    runtime.execute("connected = null;").unwrap();
    runtime.collect().unwrap();
    assert!(runtime.retain(&connected).is_ok());
}

#[test]
fn capacity() {
    let mut runtime = runtime();
    runtime.collect().unwrap();
    let mut previous = None;
    for _ in 0..24 {
        runtime.execute("document.createElement('div');").unwrap();
        runtime.collect().unwrap();
        let slots = runtime.addons().slots();
        if let Some(previous) = previous {
            assert_eq!(slots, previous);
        }
        previous = Some(slots);
    }
}

#[test]
fn scrollcontroller() {
    let mut runtime = runtime();
    runtime
        .execute(
            "let node = document.createElement('div'); let follow = scroll.attach(node).followEnd;",
        )
        .unwrap();
    runtime.collect().unwrap();
    runtime.execute("follow();").unwrap();
    runtime.execute("follow = null;").unwrap();
    runtime.collect().unwrap();
    runtime.execute("scroll.attach(node);").unwrap();
}

#[test]
fn document() {
    let mut page = super::super::loaded(&mut super::super::source());
    page.collect().unwrap();
    for _ in 0..4200 {
        page.evaluate("document.createElement('div');").unwrap();
        let collection = page.collect().unwrap();
        assert_eq!(collection.nodes, 1);
        assert_eq!(collection.document.released, collection.nodes);
        assert_eq!(collection.javascript.released, collection.objects);
        assert_eq!(
            collection.document.live + collection.document.vacant,
            collection.document.capacity
        );
    }
    page.render().unwrap();
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "button")
    );
}

#[test]
fn snapshot() {
    let mut page = super::super::loaded(&mut super::super::source());
    page.collect().unwrap();
    page.evaluate(
        "let detached = document.createElement('div'); document.root.appendChild(detached);",
    )
    .unwrap();
    page.render().unwrap();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    page.evaluate("document.root.removeChild(detached); detached = null;")
        .unwrap();
    page.render().unwrap();
    let frame = page.frame().unwrap();
    assert_eq!(page.collect().unwrap().nodes, 0);
    page.acknowledge(frame.id()).unwrap();
    assert_eq!(page.collect().unwrap().nodes, 1);
}

#[test]
fn weak() {
    let mut page = super::super::loaded(&mut super::super::source());
    page.evaluate("let map = new WeakMap(); let node = document.createElement('div'); node.setAttribute('id', 'weak'); document.root.appendChild(node); map.set(node, {owner: node}); node = null;")
        .unwrap();
    assert_eq!(page.collect().unwrap().nodes, 0);
    assert_eq!(
        page.evaluate("map.has(document.getElementById('weak'));")
            .unwrap(),
        "true"
    );
    page.render().unwrap();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    page.evaluate("document.root.removeChild(document.getElementById('weak'));")
        .unwrap();
    page.render().unwrap();
    let frame = page.frame().unwrap();
    assert_eq!(page.collect().unwrap().nodes, 0);
    page.acknowledge(frame.id()).unwrap();
    assert_eq!(page.collect().unwrap().nodes, 1);
}

#[test]
fn stale() {
    let mut page = super::super::loaded(&mut super::super::source());
    page.evaluate("let transient = document.createElement('button'); transient.setAttribute('id', 'transient'); document.root.appendChild(transient);").unwrap();
    page.render().unwrap();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    let id = page
        .inspect()
        .nodes
        .iter()
        .find(|node| {
            node.attributes
                .get("id")
                .is_some_and(|id| id == "transient")
        })
        .unwrap()
        .id;
    let hit = (0..120)
        .map(|y| page.hit(50, y))
        .find(|hit| hit.target() == Some(id))
        .unwrap();
    page.evaluate("document.root.removeChild(transient); transient = null;")
        .unwrap();
    page.render().unwrap();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    assert_eq!(page.collect().unwrap().nodes, 1);
    page.dispatch(&hit).unwrap();
    assert!(page.messages().is_empty());
}
