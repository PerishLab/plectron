#[test]
fn methods() {
    let mut page = super::page();
    assert_eq!(page.evaluate("a.setText === b.setText;").unwrap(), "true");
    assert_eq!(page.evaluate("const detached = a.setText; let caught = false; try { detached('wrong'); } catch (error) { caught = error instanceof TypeError; } caught;").unwrap(), "true");
    page.evaluate("detached.call(b, 'Borrowed');").unwrap();
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "Borrowed")
    );
    assert_eq!(page.evaluate("let rejected = false; try { detached.call({}, 'wrong'); } catch (error) { rejected = error instanceof TypeError; } rejected;").unwrap(), "true");
    page.evaluate("a.setText = (value) => { bridge.postMessage(value); }; a.setText('override'); b.setText('original');").unwrap();
    assert_eq!(page.messages(), ["override"]);
    assert!(
        page.inspect()
            .nodes
            .iter()
            .any(|node| node.content == "original")
    );
    assert_eq!(page.evaluate("a.setText === b.setText;").unwrap(), "false");
}

#[test]
fn sharing() {
    let mut page = super::page();
    let ids = (0..1000)
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(page.evaluate(&format!("let shared = true; for (const id of [{ids}]) {{ const node = document.createElement('div'); if (node.setText !== a.setText) {{ shared = false; }} }} shared;")).unwrap(), "true");
    let mut other = super::page();
    page.evaluate("a.setText = () => 'local';").unwrap();
    assert_eq!(other.evaluate("a.setText === b.setText;").unwrap(), "true");
}

#[test]
fn declarations() {
    use shared::cache::{Command, Policy};
    let mut page = super::page();
    page.evaluate(
        "a.setAttribute('style', 'height:12px;'); b.setAttribute('style', 'height:12px;');",
    )
    .unwrap();
    let cache = page
        .inspect()
        .caches
        .into_iter()
        .find(|cache| cache.name == "declarations")
        .unwrap();
    assert_eq!((cache.stats.inserted, cache.stats.hits), (1, 1));
    for command in [
        Command::Clear,
        Command::Configure {
            policy: Policy {
                read: false,
                write: false,
                entries: 1,
                bytes: 1024,
            },
        },
        Command::Configure {
            policy: Policy::bounded(1, 1024),
        },
        Command::Destroy,
    ] {
        page.cache(cache.id, command).unwrap();
        page.evaluate(
            "a.setAttribute('style', 'height:13px;'); a.setAttribute('style', 'height:12px;');",
        )
        .unwrap();
        let before = page.inspect().revision;
        assert!(
            page.evaluate("a.setAttribute('style', 'height:invalid;');")
                .is_err()
        );
        assert_eq!(page.inspect().revision, before);
        page.render().unwrap();
        let node = page
            .inspect()
            .nodes
            .iter()
            .find(|node| node.attributes.get("id").is_some_and(|id| id == "a"))
            .unwrap()
            .clone();
        assert_eq!(node.attributes.get("style").unwrap(), "height:12px;");
    }
}
