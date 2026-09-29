use domengine::{
    Document, Selection,
    change::{Attribute, Fact},
};

#[test]
fn facts() {
    let mut document = Document::default();
    let start = document.version();
    let parent = document.element("div").unwrap();
    document.append(document.root(), parent).unwrap();
    let text = document.text("A").unwrap();
    document.append(parent, text).unwrap();
    document.attribute(parent, "class", "old").unwrap();
    document.replace(text, "B").unwrap();
    let changes = document.changes(start).unwrap();
    assert_eq!(changes.len(), 4);
    assert_eq!(
        changes[0].fact,
        Fact::Attached {
            node: parent,
            from: None,
            to: Some(document.root())
        }
    );
    assert_eq!(
        changes[2].fact,
        Fact::Attribute {
            node: parent,
            name: Attribute::Class
        }
    );
    assert_eq!(changes[3].fact, Fact::Text { node: text });
    assert!(
        changes
            .windows(2)
            .all(|pair| pair[1].revision == pair[0].revision + 1)
    );
    assert_eq!(document.changes(start).unwrap(), changes);
    let unchanged = document.version();
    document.attribute(parent, "class", "old").unwrap();
    document.replace(text, "B").unwrap();
    assert!(document.append(text, parent).is_err());
    assert!(document.attribute(text, "class", "invalid").is_err());
    assert_eq!(document.version(), unchanged);
    document.remove(parent, text).unwrap();
    assert_eq!(
        document.changes(unchanged).unwrap()[0].fact,
        Fact::Attached {
            node: text,
            from: Some(parent),
            to: None
        }
    );
}

#[test]
fn detached() {
    let mut document = Document::default();
    let start = document.version();
    let parent = document.element("div").unwrap();
    document.attribute(parent, "class", "record").unwrap();
    let text = document.text("A").unwrap();
    document.append(parent, text).unwrap();
    document.replace(text, "B").unwrap();
    assert_eq!(document.version(), start);
    assert!(document.changes(start).unwrap().is_empty());
    document.append(document.root(), parent).unwrap();
    let changes = document.changes(start).unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0].fact,
        Fact::Attached {
            node: parent,
            from: None,
            to: Some(document.root())
        }
    );
}

#[test]
fn editing() {
    let mut document = Document::default();
    let input = document.element("input").unwrap();
    document.append(document.root(), input).unwrap();
    let start = document.version();
    document.value(input, "ABC").unwrap();
    document.select(input, Selection::caret(1)).unwrap();
    document.focus(Some(input)).unwrap();
    let facts: Vec<_> = document
        .changes(start)
        .unwrap()
        .into_iter()
        .map(|change| change.fact)
        .collect();
    assert_eq!(
        facts,
        [
            Fact::Value { node: input },
            Fact::Selection { node: input },
            Fact::Selection { node: input },
            Fact::Focus {
                from: None,
                to: Some(input)
            }
        ]
    );
    let before = document.version();
    document.remove(document.root(), input).unwrap();
    let changes = document.changes(before).unwrap();
    assert!(changes.iter().any(|change| change.fact
        == Fact::Focus {
            from: Some(input),
            to: None
        }));
}

#[test]
fn history() {
    let mut document = Document::default();
    let config = config::Store::default();
    config.set("dom.history", 1024.into());
    document.configure(&config).unwrap();
    let start = document.version();
    let root = document.root();
    for index in 0..1024 {
        document.attribute(root, "id", &index.to_string()).unwrap();
    }
    assert_eq!(document.changes(start).unwrap().len(), 1024);
    let recent = document.version();
    document.attribute(root, "id", "overflow").unwrap();
    assert!(document.changes(start).is_none());
    assert_eq!(document.changes(recent).unwrap().len(), 1);
    assert!(document.changes(Document::default().version()).is_none());
    assert!(document.changes(document.version()).unwrap().is_empty());
}
use domengine::html;
use domengine::ime::{Composition, Effect, Emphasis, Id, Input, Span};

#[test]
fn projection() {
    let mut document = html::parse("<body><input id='a'><input id='b'></body>").unwrap();
    let a = document.find("a").unwrap();
    let b = document.find("b").unwrap();
    document.value(a, "A中").unwrap();
    document.focus(Some(a)).unwrap();
    let id = Id(7);
    let update = |text: &str, focus| {
        Input::Update(Composition {
            id,
            replacement: Selection {
                anchor: 1,
                focus: 2,
            },
            text: text.into(),
            selection: Selection::caret(focus),
            spans: vec![Span {
                start: 0,
                end: text.chars().count(),
                emphasis: Emphasis::Composition,
            }],
        })
    };
    assert_eq!(
        document.compose(a, &update("zhong", 5)).unwrap(),
        Effect::Updated
    );
    let buffer = document.buffer(a).unwrap();
    assert_eq!(buffer.text(), "A中");
    assert_eq!(buffer.projection().text, "Azhong");
    assert_eq!(buffer.projection().offsets, [6, 6]);
    assert_eq!(
        document
            .compose(
                a,
                &Input::Update(Composition {
                    replacement: Selection::caret(0),
                    ..match update("钟", 1) {
                        Input::Update(value) => value,
                        _ => unreachable!(),
                    }
                })
            )
            .unwrap_err()
            .0,
        "composition target changed during session"
    );
    assert_eq!(document.buffer(a).unwrap().projection().text, "Azhong");
    assert_eq!(
        document
            .compose(
                a,
                &Input::Commit {
                    id: Id(8),
                    text: "钟".into(),
                }
            )
            .unwrap(),
        Effect::Ignored
    );
    assert_eq!(
        document
            .compose(
                a,
                &Input::Commit {
                    id,
                    text: "钟".into(),
                }
            )
            .unwrap(),
        Effect::Committed { changed: true }
    );
    assert_eq!(document.buffer(a).unwrap().text(), "A钟");
    assert_eq!(document.buffer(a).unwrap().selection(), Selection::caret(2));

    document.compose(a, &update("temporary", 9)).unwrap();
    document.focus(Some(b)).unwrap();
    assert!(document.buffer(a).unwrap().composition().is_none());
    assert_eq!(document.buffer(a).unwrap().text(), "A钟");
}

#[test]
fn mark() {
    let mut document = html::parse("<body><input id='a'></body>").unwrap();
    let node = document.find("a").unwrap();
    document.value(node, "A中").unwrap();
    document.select(node, Selection::caret(1)).unwrap();
    let id = Id(9);
    document
        .compose(
            node,
            &Input::Mark {
                id,
                text: "zhong".into(),
                selection: Selection::caret(5),
            },
        )
        .unwrap();
    assert_eq!(document.buffer(node).unwrap().projection().text, "Azhong中");
    document
        .compose(
            node,
            &Input::Mark {
                id,
                text: "钟".into(),
                selection: Selection::caret(1),
            },
        )
        .unwrap();
    document
        .compose(
            node,
            &Input::Commit {
                id,
                text: "钟".into(),
            },
        )
        .unwrap();
    assert_eq!(document.buffer(node).unwrap().text(), "A钟中");
}
