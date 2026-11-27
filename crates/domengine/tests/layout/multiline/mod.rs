mod caret;
use ::fixtures::Text;
use domengine::{
    Constraints, Direction, Document, Edit, Extent, Input, Layout, Node, Selection, Sheet, html,
    ime,
};

pub(super) struct Harness {
    pub document: Document,
    pub node: Node,
}

impl Harness {
    pub(super) fn new(style: &str, multiline: bool) -> Self {
        let source = format!("<div id='editor' style='{style}'></div>");
        let mut document = html::parse(&source).unwrap();
        let node = document.find("editor").unwrap();
        let text = document.text("").unwrap();
        let caret = document.element("p-caret").unwrap();
        document.append(node, text).unwrap();
        document.append(node, caret).unwrap();
        document.enable(node, text, caret).unwrap();
        if multiline {
            document.multiline(node, true).unwrap();
        }
        document.focus(Some(node)).unwrap();
        Self { document, node }
    }

    pub(super) fn layout(&self, extent: Extent) -> Layout {
        let input = Input::capture(&self.document, &Sheet::default()).unwrap();
        Layout::build(&input, Constraints::viewport(extent), &Text).unwrap()
    }
}

#[test]
fn mode() {
    let mut single = html::parse("<input id='a'>").unwrap();
    let input = single.find("a").unwrap();
    assert!(single.value(input, "a\nb").is_err());
    assert!(single.edit(input, &Edit::Insert("a\nb".into())).is_err());
    assert!(!single.edit(input, &Edit::Newline).unwrap());
    assert_eq!(single.buffer(input).unwrap().text(), "");

    let mut multi = Harness::new("width: 120px;", true);
    let node = multi.node;
    multi.document.value(node, "one\r\ntwo\rthree").unwrap();
    assert_eq!(
        multi.document.buffer(node).unwrap().text(),
        "one\ntwo\nthree"
    );
    assert!(multi.document.value(node, "bad\0text").is_err());
    assert!(multi.document.value(node, "bad\ttext").is_err());
    multi.document.caret(node, 3).unwrap();
    assert!(multi.document.edit(node, &Edit::Newline).unwrap());
    assert_eq!(
        multi.document.buffer(node).unwrap().text(),
        "one\n\ntwo\nthree"
    );
    assert!(multi.document.multiline(node, false).is_err());
    let large = "a".repeat(16_384);
    multi.document.value(node, &large).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().text().len(), 16_384);
    assert!(
        multi
            .document
            .edit(node, &Edit::Insert("x".into()))
            .is_err()
    );
    multi.document.value(node, "plain").unwrap();
    multi.document.multiline(node, false).unwrap();
    assert!(multi.document.value(node, "a\nb").is_err());
}

#[test]
fn vertical() {
    let mut multi = Harness::new("width: 160px;", true);
    let node = multi.node;
    multi.document.value(node, "abcdef\nxy\n\n123456").unwrap();
    multi.document.caret(node, 5).unwrap();
    let down = Edit::Move {
        direction: Direction::Down,
        extend: false,
    };
    let up = Edit::Move {
        direction: Direction::Up,
        extend: false,
    };
    multi.document.edit(node, &down).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 9);
    multi.document.edit(node, &down).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 10);
    multi.document.edit(node, &down).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 16);
    multi.document.edit(node, &up).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 10);
    multi.document.edit(node, &up).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 9);
    multi.document.edit(node, &up).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 5);
    multi
        .document
        .edit(
            node,
            &Edit::Move {
                direction: Direction::Down,
                extend: true,
            },
        )
        .unwrap();
    assert_eq!(
        multi.document.buffer(node).unwrap().selection(),
        Selection {
            anchor: 5,
            focus: 9
        }
    );
    multi
        .document
        .edit(
            node,
            &Edit::Move {
                direction: Direction::Start,
                extend: false,
            },
        )
        .unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 7);
    multi
        .document
        .edit(
            node,
            &Edit::Move {
                direction: Direction::End,
                extend: false,
            },
        )
        .unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 9);
    multi.document.value(node, "A😀\n中X").unwrap();
    multi.document.caret(node, 1).unwrap();
    multi.document.edit(node, &down).unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().caret(), 4);
}

#[test]
fn clipboard() {
    let mut multi = Harness::new("width: 140px;", true);
    let node = multi.node;
    multi.document.value(node, "alpha\nbeta").unwrap();
    multi
        .document
        .select(
            node,
            Selection {
                anchor: 2,
                focus: 8,
            },
        )
        .unwrap();
    assert_eq!(multi.document.buffer(node).unwrap().selected(), "pha\nbe");
    assert!(multi.document.edit(node, &Edit::Cut).unwrap());
    assert_eq!(multi.document.buffer(node).unwrap().text(), "alta");
    assert!(
        multi
            .document
            .edit(node, &Edit::Paste(Some("1\r\n2\0\t3".into())))
            .unwrap()
    );
    assert_eq!(multi.document.buffer(node).unwrap().text(), "al1\n23ta");

    let mut single = html::parse("<input id='a'>").unwrap();
    let input = single.find("a").unwrap();
    single.value(input, "hello").unwrap();
    single
        .select(
            input,
            Selection {
                anchor: 1,
                focus: 4,
            },
        )
        .unwrap();
    assert_eq!(single.buffer(input).unwrap().selected(), "ell");
    assert!(
        single
            .edit(input, &Edit::Paste(Some("A\r\nB\0C".into())))
            .unwrap()
    );
    assert_eq!(single.buffer(input).unwrap().text(), "hABCo");
}

#[test]
fn composition() {
    let mut multi = Harness::new("width: 140px;", true);
    let node = multi.node;
    multi.document.value(node, "hello\nworld").unwrap();
    let id = ime::Id(7);
    let mark = ime::Input::Update(ime::Composition {
        id,
        replacement: Selection {
            anchor: 3,
            focus: 8,
        },
        text: "你好".into(),
        selection: Selection::caret(2),
        spans: Vec::new(),
    });
    assert_eq!(
        multi.document.compose(node, &mark).unwrap(),
        ime::Effect::Updated
    );
    assert_eq!(
        multi.document.buffer(node).unwrap().projection().text,
        "hel你好rld"
    );
    let layout = multi.layout(Extent {
        width: 160,
        height: 80,
    });
    graphics::shared::Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap();
    assert_eq!(
        multi
            .document
            .compose(node, &ime::Input::Cancel { id })
            .unwrap(),
        ime::Effect::Cancelled
    );
    assert_eq!(multi.document.buffer(node).unwrap().text(), "hello\nworld");
    multi.document.compose(node, &mark).unwrap();
    assert_eq!(
        multi
            .document
            .compose(
                node,
                &ime::Input::Commit {
                    id,
                    text: "甲\r\n乙".into(),
                }
            )
            .unwrap(),
        ime::Effect::Committed { changed: true }
    );
    assert_eq!(multi.document.buffer(node).unwrap().text(), "hel甲\n乙rld");

    let large = "x".repeat(5000);
    multi.document.value(node, &large).unwrap();
    multi.document.caret(node, 5000).unwrap();
    let next = ime::Id(8);
    multi
        .document
        .compose(
            node,
            &ime::Input::Mark {
                id: next,
                text: "中".into(),
                selection: Selection::caret(1),
            },
        )
        .unwrap();
    multi
        .document
        .compose(
            node,
            &ime::Input::Commit {
                id: next,
                text: "中".into(),
            },
        )
        .unwrap();
    assert_eq!(
        multi.document.buffer(node).unwrap().text().chars().count(),
        5001
    );

    let mut single = html::parse("<input id='a'>").unwrap();
    let input = single.find("a").unwrap();
    single
        .compose(
            input,
            &ime::Input::Mark {
                id: ime::Id(9),
                text: "a".into(),
                selection: Selection::caret(1),
            },
        )
        .unwrap();
    assert!(
        single
            .compose(
                input,
                &ime::Input::Commit {
                    id: ime::Id(9),
                    text: "a\nb".into(),
                }
            )
            .is_err()
    );
}
