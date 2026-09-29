use super::{Harness, constraints};
use ::fixtures::Text;
use domengine::{Fault, Input, Layout, Sheet, html};
use shared::cache::Command;

#[test]
fn lifetime() {
    let mut document = domengine::Document::default();
    let parent = document.element("div").unwrap();
    let child = document.text("hello").unwrap();
    document.append(parent, child).unwrap();
    document.append(document.root(), parent).unwrap();
    assert!(document.collect([]).is_empty());
    document.remove(document.root(), parent).unwrap();
    assert!(document.collect([child]).is_empty());
    assert_eq!(document.collect([]).len(), 2);
    assert_eq!(
        document.collection(),
        domengine::Collection {
            live: 1,
            released: 2,
            capacity: 3,
            vacant: 2,
            allocations: 2,
        }
    );
    assert!(document.entry(parent).is_err());
    assert!(document.entry(child).is_err());
    let fresh = document.element("span").unwrap();
    assert!(fresh.slot() == parent.slot() || fresh.slot() == child.slot());
    assert_ne!(fresh.id(), parent.id());
    assert_ne!(fresh.id(), child.id());
    assert!(document.entry(parent).is_err());
}

#[test]
fn reuse() {
    let mut document = domengine::Document::default();
    for _ in 0..5000 {
        let node = document.element("div").unwrap();
        assert_eq!(document.collect([]), [node]);
        let report = document.collection();
        assert_eq!(report.live, 1);
        assert_eq!(report.released, 1);
        assert_eq!(report.capacity, 2);
        assert_eq!(report.vacant, 1);
    }
    assert_eq!(document.collection().allocations, 5000);
}

#[test]
fn failure() {
    let mut test = Harness::new("<div id='a'><p>ABC</p></div>", "");
    test.check();
    let committed = test.capture.inspect().committed;
    let a = test.document.find("a").unwrap();
    test.document
        .attribute(a, "style", "overflow: auto;")
        .unwrap();
    let input = Input::capture(&test.document, &Sheet::default()).unwrap();
    let expected = Layout::build(&input, constraints(), &Text).unwrap_err();
    for _ in 0..2 {
        let actual = test
            .capture
            .update(&test.document, |input| {
                test.pipeline.build(input, constraints())
            })
            .unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(test.capture.inspect().committed, committed);
    }
    test.document
        .attribute(a, "style", "color: #ff0000;")
        .unwrap();
    let rejected = test.capture.update(&test.document, |_| {
        Err::<(), _>(Fault("scene rejected".into()))
    });
    assert!(rejected.is_err());
    assert_eq!(test.capture.inspect().committed, committed);
    test.check();
    assert_eq!(test.capture.inspect().changes, 2);
    assert!(test.capture.inspect().failure.is_none());
}

#[test]
fn reset() {
    let mut test = Harness::new("<p id='a'>ABC</p>", "");
    let config = config::Store::default();
    config.set("dom.history", 1024.into());
    test.document.configure(&config).unwrap();
    test.check();
    let a = test.document.find("a").unwrap();
    for index in 0..1025 {
        test.document
            .attribute(a, "class", &index.to_string())
            .unwrap();
    }
    test.check();
    assert_eq!(test.capture.inspect().reset.as_deref(), Some("history"));
    assert_eq!(test.capture.inspect().rebuilt, test.capture.inspect().nodes);
    test.document = html::parse("<p>Other</p>").unwrap();
    test.check();
    assert_eq!(test.capture.inspect().reset.as_deref(), Some("document"));
    test.rules = "p { color: #ff0000; }".into();
    test.capture.replace(Sheet::parse(&test.rules).unwrap());
    test.check();
    assert_eq!(test.capture.inspect().reset.as_deref(), Some("stylesheet"));
    test.capture.clear();
    assert_eq!(test.capture.inspect().committed, None);
    test.check();
    assert_eq!(test.capture.inspect().reset.as_deref(), Some("cleared"));
}

#[test]
fn ownership() {
    let mut test = Harness::new("<p id='a'>ABC</p>", "");
    let snapshot = test
        .capture
        .update(&test.document, |input| Ok(input.clone()))
        .unwrap();
    let before = Layout::build(&snapshot, constraints(), &Text).unwrap();
    let a = test.document.find("a").unwrap();
    test.document.replace(a, "Changed").unwrap();
    test.check();
    for cache in test.manager.inspect() {
        test.manager.command(cache.id, Command::Destroy).unwrap();
    }
    test.capture.enable(false);
    test.check();
    assert_eq!(test.capture.inspect().rebuilt, test.capture.inspect().nodes);
    assert_eq!(test.pipeline.inspect().hits, 0);
    test.capture.enable(true);
    test.check();
    assert_eq!(test.capture.inspect().rebuilt, 0);
    test.capture.close();
    let replay = Layout::build(&snapshot, constraints(), &Text).unwrap();
    assert_eq!(replay.fragments(), before.fragments());
    assert_eq!(super::pixels(&replay), super::pixels(&before));
}

#[test]
fn validation() {
    let mut test = Harness::new(
        "<div id='root' style='display: none;'><p id='child'>ABC</p></div>",
        "",
    );
    test.check();
    let committed = test.capture.inspect().committed;
    let child = test.document.find("child").unwrap();
    test.document
        .attribute(child, "style", "width: broken;")
        .unwrap();
    for _ in 0..2 {
        let expected = Input::capture(&test.document, &Sheet::default())
            .err()
            .unwrap();
        let actual = test.capture.update(&test.document, |_| Ok(())).unwrap_err();
        assert_eq!(actual, expected);
        assert_eq!(test.capture.inspect().committed, committed);
    }
    test.document
        .attribute(child, "style", "width: 32px;")
        .unwrap();
    test.check();
    let root = test.document.find("root").unwrap();
    test.document
        .attribute(root, "style", "display: block;")
        .unwrap();
    test.check();
}
