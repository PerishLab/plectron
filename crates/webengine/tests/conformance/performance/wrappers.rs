#[path = "../../../src/addons/wrappers.rs"]
mod binding;
use binding::Wrappers;
use jsengine::{Engine, Value};
use std::collections::BTreeMap;

#[test]
fn identity() {
    let mut engine = Engine::default();
    let mut foreign = Engine::default();
    let node = domengine::Document::default().root();
    let value = engine.object(BTreeMap::new()).unwrap();
    let other = foreign.object(BTreeMap::new()).unwrap();
    let mut wrappers = Wrappers::default();
    wrappers.insert(node, value.clone());
    wrappers.insert(node, value.clone());
    assert_eq!(wrappers.node(&value).unwrap(), node);
    assert_eq!(wrappers.get(&node), Some(&value));
    assert!(wrappers.node(&other).is_err());
    assert!(wrappers.node(&Value::Null).is_err());
    assert!(Wrappers::default().node(&value).is_err());
    let next = engine.object(BTreeMap::new()).unwrap();
    wrappers.insert(node, next.clone());
    assert!(wrappers.node(&value).is_err());
    assert_eq!(wrappers.node(&next).unwrap(), node);
    let mut roots = Vec::new();
    wrappers.roots(&mut |root| roots.push(root));
    assert_eq!(roots, [node]);
    let Value::Object(handle) = next else {
        unreachable!()
    };
    wrappers.sweep(&[handle]);
    assert!(wrappers.get(&node).is_none());
    assert!(wrappers.node(&Value::Object(handle)).is_err());
    roots.clear();
    wrappers.roots(&mut |root| roots.push(root));
    assert!(roots.is_empty());
}

#[test]
fn lifecycle() {
    let mut page = super::loaded(&mut super::source());
    assert_eq!(
        page.evaluate("button === document.getElementById('button');")
            .unwrap(),
        "true"
    );
    page.evaluate("const parent = document.getElementById('body'); parent.removeChild(button); parent.appendChild(button);").unwrap();
    assert_eq!(
        page.evaluate("button === document.getElementById('button');")
            .unwrap(),
        "true"
    );
    assert!(
        page.evaluate("parent.appendChild({});")
            .unwrap_err()
            .0
            .contains("expected a node")
    );
    page.render().unwrap();
    page.close();
    assert!(page.evaluate("parent.appendChild(button);").is_err());
}
