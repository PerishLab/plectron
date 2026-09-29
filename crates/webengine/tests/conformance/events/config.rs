use ::config::Store;
use webengine::Page;

fn page(config: &Store) -> Page {
    let mut source = super::super::source();
    source.files.get_mut("/site/main.js").unwrap().body = "const node = document.getElementById('button'); let calls = 0; const callback = () => { calls = calls + 1; };".into();
    let mut page = Page::configured(
        "app://demo/site/index.html",
        domengine::Extent {
            width: 64,
            height: 64,
        },
        config.clone(),
    )
    .unwrap();
    page.load(&mut source).unwrap();
    page
}

#[test]
fn scopes() {
    let config = Store::default();
    config.set("events.scopes", 1.into());
    let mut page = page(&config);
    assert_eq!(
        page.evaluate(
            "const first = events.scope(); node.addEventListener('test', callback, {scope: first});",
        )
        .unwrap(),
        "undefined"
    );
    assert!(
        page.evaluate("events.scope();")
            .unwrap_err()
            .0
            .contains("scope budget")
    );
    config.set("events.scopes", 0.into());
    page.evaluate("node.dispatchEvent('test', '');").unwrap();
    assert_eq!(page.evaluate("calls;").unwrap(), "1");
    page.evaluate("first.dispose(); node.dispatchEvent('test', '');")
        .unwrap();
    assert_eq!(page.evaluate("calls;").unwrap(), "1");
    assert!(page.evaluate("events.scope();").is_err());
    config.set("events.scopes", 1.into());
    page.evaluate("const second = events.scope(); second.dispose();")
        .unwrap();
    assert!(page.evaluate("first.dispose();").is_err());
    page.evaluate("events.scope();").unwrap();
}

#[test]
fn subscriptions() {
    let config = Store::default();
    config.set("events.subscriptions", 1.into());
    config.set("events.node", 1.into());
    let mut page = page(&config);
    page.evaluate("const lease = node.addEventListener('test', callback);")
        .unwrap();
    assert!(
        page.evaluate("node.addEventListener('test', callback);")
            .is_err()
    );
    config.set("events.subscriptions", 2.into());
    assert!(
        page.evaluate("node.addEventListener('test', callback);")
            .is_err()
    );
    config.set("events.node", 2.into());
    page.evaluate("node.addEventListener('test', callback, {once: true});")
        .unwrap();
    config.set("events.subscriptions", 0.into());
    page.evaluate("node.dispatchEvent('test', ''); lease.dispose();")
        .unwrap();
    assert_eq!(page.evaluate("calls;").unwrap(), "2");
    config.set("events.subscriptions", 1.into());
    page.evaluate("const scope = events.scope(); node.addEventListener('test', callback, {scope: scope}); scope.dispose(); node.addEventListener('test', callback);").unwrap();
}

#[test]
fn invalid() {
    let config = Store::default();
    let mut page = page(&config);
    for key in ["events.scopes", "events.subscriptions", "events.node"] {
        let source = if key == "events.scopes" {
            "events.scope();"
        } else {
            "node.addEventListener('test', callback);"
        };
        for value in [
            (-1).into(),
            true.into(),
            "bad".into(),
            ::config::Value::Null,
        ] {
            config.set(key, value.clone());
            assert_eq!(config.get(key), Some(value));
            assert!(page.evaluate(source).unwrap_err().0.contains(key));
        }
        config.remove(key);
        page.evaluate(source).unwrap();
    }
}
