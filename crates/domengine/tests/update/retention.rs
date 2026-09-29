use super::{Harness, pixels};
use domengine::Document;

#[test]
fn configure() {
    let config = config::Store::default();
    config.set("dom.history", 4.into());
    let mut document = Document::default();
    document.configure(&config).unwrap();
    let start = document.version();
    for index in 0..4 {
        document
            .attribute(document.root(), "id", &index.to_string())
            .unwrap();
    }
    assert_eq!(document.changes(start).unwrap().len(), 4);
    let changes = document.changes(start).unwrap();
    config.set("dom.history", 1.into());
    document.configure(&config).unwrap();
    assert!(document.changes(start).is_none());
    assert_eq!(document.retention().entries, 1);
    assert_eq!(document.retention().floor, 3);
    assert_eq!(changes.len(), 4);
    config.set("dom.history", 8.into());
    document.configure(&config).unwrap();
    assert!(document.changes(start).is_none());
    let recent = document.version();
    document.attribute(document.root(), "id", "next").unwrap();
    assert_eq!(document.changes(recent).unwrap().len(), 1);
    config.set("dom.history", 0.into());
    document.configure(&config).unwrap();
    document
        .attribute(document.root(), "id", "disabled")
        .unwrap();
    assert!(document.changes(recent).is_none());
    assert!(document.changes(document.version()).unwrap().is_empty());
    assert_eq!(document.retention().entries, 0);
    assert_eq!(document.retention().bytes, 0);
    config.remove("dom.history");
    document.configure(&config).unwrap();
    assert_eq!(
        document.retention().limit,
        Document::default().retention().limit
    );
}

#[test]
fn bounded() {
    let config = config::Store::default();
    config.set("dom.history", 4.into());
    let mut document = Document::default();
    document.configure(&config).unwrap();
    for index in 0..4 {
        document
            .attribute(document.root(), "id", &index.to_string())
            .unwrap();
    }
    let full = document.retention();
    let previous = document.version();
    for index in 4..128 {
        document
            .attribute(document.root(), "id", &index.to_string())
            .unwrap();
    }
    assert_eq!(document.retention().entries, full.entries);
    assert_eq!(document.retention().bytes, full.bytes);
    assert!(document.changes(previous).is_none());
    let recent = document.version();
    document.attribute(document.root(), "id", "latest").unwrap();
    assert_eq!(document.changes(recent).unwrap().len(), 1);
}

#[test]
fn invalid() {
    let config = config::Store::default();
    let mut document = Document::default();
    let before = document.retention().limit;
    for value in [(-1).into(), true.into(), "bad".into(), config::Value::Null] {
        config.set("dom.history", value.clone());
        assert!(
            document
                .configure(&config)
                .unwrap_err()
                .0
                .contains("dom.history")
        );
        assert_eq!(config.get("dom.history"), Some(value));
        assert_eq!(document.retention().limit, before);
    }
}

#[test]
fn append() {
    let mut test = Harness::new(
        &format!("<div id='root'>{}</div>", "<p>A</p>".repeat(24)),
        "p { height: 16px; }",
    );
    let original = test.check();
    let image = pixels(&original);
    let root = test.document.find("root").unwrap();
    for _ in 0..300 {
        let node = test.document.element("p").unwrap();
        test.document.replace(node, "B").unwrap();
        test.document.append(root, node).unwrap();
    }
    test.check();
    let report = test.capture.inspect();
    assert!(report.changes >= 300);
    assert_eq!(report.reset.as_deref(), Some("changes"));
    assert_eq!(report.reused, 0);
    assert_eq!(pixels(&original), image);
    let config = config::Store::default();
    config.set("dom.history", 0.into());
    test.document.configure(&config).unwrap();
    test.document.replace(root, "Replaced").unwrap();
    test.check();
    assert_eq!(test.capture.inspect().reset.as_deref(), Some("history"));
}
