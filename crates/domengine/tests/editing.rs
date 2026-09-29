use ::fixtures::Text;
use domengine::{Constraints, Document, Edit, Extent, Input, Layout, Sheet, html};
use domengine::{Direction, Selection};

#[test]
fn selection() {
    let mut document = html::parse("<input id='a'>").unwrap();
    let node = document.find("a").unwrap();
    document.value(node, "A😀fi中").unwrap();
    document
        .select(
            node,
            Selection {
                anchor: 4,
                focus: 1,
            },
        )
        .unwrap();
    assert_eq!(document.buffer(node).unwrap().offsets(), [7, 1]);
    let revision = document.revision();
    assert!(
        document
            .select(
                node,
                Selection {
                    anchor: 6,
                    focus: 1
                }
            )
            .is_err()
    );
    assert_eq!(document.revision(), revision);
    document
        .edit(
            node,
            &Edit::Move {
                direction: Direction::Right,
                extend: true,
            },
        )
        .unwrap();
    assert_eq!(
        document.buffer(node).unwrap().selection(),
        Selection {
            anchor: 4,
            focus: 2
        }
    );
    document.edit(node, &Edit::Insert("X".into())).unwrap();
    assert_eq!(document.buffer(node).unwrap().text(), "A😀X中");
    assert_eq!(
        document.buffer(node).unwrap().selection(),
        Selection::caret(3)
    );
    document.edit(node, &Edit::All).unwrap();
    document
        .edit(
            node,
            &Edit::Move {
                direction: Direction::Left,
                extend: false,
            },
        )
        .unwrap();
    assert_eq!(
        document.buffer(node).unwrap().selection(),
        Selection::caret(0)
    );
    document.edit(node, &Edit::All).unwrap();
    document
        .edit(
            node,
            &Edit::Move {
                direction: Direction::Right,
                extend: false,
            },
        )
        .unwrap();
    assert_eq!(
        document.buffer(node).unwrap().selection(),
        Selection::caret(4)
    );
    document.edit(node, &Edit::All).unwrap();
    document.edit(node, &Edit::Backward).unwrap();
    assert_eq!(document.buffer(node).unwrap().text(), "");
    document.value(node, &"x".repeat(4096)).unwrap();
    document.edit(node, &Edit::All).unwrap();
    document.edit(node, &Edit::Insert("😀".into())).unwrap();
    assert_eq!(document.buffer(node).unwrap().text(), "😀");
    assert_eq!(
        document.buffer(node).unwrap().selection(),
        Selection::caret(1)
    );
}

#[test]
fn viewport() {
    let mut document = html::parse("<div style='width: 80px; height: 24px; translate: 4px 4px;'><input id='a' style='width: 64px;'></div>").unwrap();
    let node = document.find("a").unwrap();
    document.value(node, "abcdefghijklmnop").unwrap();
    document.focus(Some(node)).unwrap();
    document
        .select(
            node,
            Selection {
                anchor: 5,
                focus: 16,
            },
        )
        .unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let extent = Extent {
        width: 100,
        height: 40,
    };
    let layout = Layout::build(&input, Constraints::viewport(extent), &Text).unwrap();
    let scene =
        graphics::shared::Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap();
    let frame = graphics::shared::Frame::new(scene, graphics::shared::Visual::new(extent)).unwrap();
    let caret = layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, domengine::Content::Caret { .. }))
        .unwrap();
    let mapped = frame.map(caret.space, caret.rect).unwrap();
    assert_eq!(mapped.x, 67);
    assert_eq!(mapped.width, 1);
    assert_eq!(layout.pick(&frame, 67, 8), Some(node));
    let mark = layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, domengine::Content::Selection { .. }))
        .unwrap();
    assert!(frame.map(mark.space, mark.rect).unwrap().width <= 64);
    assert_eq!(frame.mapping(caret.space).unwrap().clip.x, 4);
    assert_eq!(
        document.buffer(node).unwrap().selection(),
        Selection {
            anchor: 5,
            focus: 16
        }
    );
}

#[test]
fn ownership() {
    let mut document =
        html::parse("<body id='body'><div id='group'><input id='a'></div><input id='b'></body>")
            .unwrap();
    let a = document.find("a").unwrap();
    let b = document.find("b").unwrap();
    let body = document.find("body").unwrap();
    let group = document.find("group").unwrap();
    document.focus(Some(a)).unwrap();
    document.value(a, "A😀中").unwrap();
    assert!(document.entry(a).unwrap().children.is_empty());
    assert_eq!(document.buffer(a).unwrap().caret(), 3);
    document
        .edit(
            a,
            &Edit::Move {
                direction: domengine::Direction::Left,
                extend: false,
            },
        )
        .unwrap();
    document.edit(a, &Edit::Backward).unwrap();
    assert_eq!(document.buffer(a).unwrap().text(), "A中");
    document.edit(a, &Edit::Insert("fi".into())).unwrap();
    assert_eq!(document.buffer(a).unwrap().text(), "Afi中");
    document.caret(a, 0).unwrap();
    document.edit(a, &Edit::Forward).unwrap();
    assert_eq!(document.buffer(a).unwrap().text(), "fi中");
    document.append(body, a).unwrap();
    assert_eq!(document.focused(), Some(a));
    document.append(group, a).unwrap();
    document.remove(body, group).unwrap();
    assert_eq!(document.focused(), None);
    assert!(document.focus(Some(a)).is_err());
    assert_eq!(document.buffer(a).unwrap().text(), "fi中");
    document.focus(Some(b)).unwrap();
    document.replace(body, "gone").unwrap();
    assert_eq!(document.focused(), None);
    let mut other = Document::default();
    assert!(other.focus(Some(a)).is_err());
    assert!(other.value(a, "foreign").is_err());
    assert!(document.append(a, group).is_err());
    assert!(document.replace(a, "children").is_err());
}

#[test]
fn atomicity() {
    let mut document = html::parse("<input id='a'>").unwrap();
    let node = document.find("a").unwrap();
    document.value(node, &"x".repeat(4096)).unwrap();
    let before = document.buffer(node).unwrap().clone();
    let revision = document.revision();
    assert!(document.edit(node, &Edit::Insert("y".into())).is_err());
    assert!(document.value(node, "line\nbreak").is_err());
    assert!(document.caret(node, 4097).is_err());
    assert_eq!(document.buffer(node).unwrap(), &before);
    assert_eq!(document.revision(), revision);
    document
        .edit(
            node,
            &Edit::Move {
                direction: domengine::Direction::End,
                extend: false,
            },
        )
        .unwrap();
    assert_eq!(document.revision(), revision);
    assert!(!document.edit(node, &Edit::Insert(String::new())).unwrap());
    document.value(node, "e\u{301}").unwrap();
    document.edit(node, &Edit::Backward).unwrap();
    assert_eq!(document.buffer(node).unwrap().text(), "e");
}

#[test]
fn projection() {
    let mut document = html::parse("<input id='a' style='width: 64px;'>").unwrap();
    let node = document.find("a").unwrap();
    document.value(node, "fi").unwrap();
    document.caret(node, 1).unwrap();
    document.focus(Some(node)).unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    document.value(node, "changed").unwrap();
    let layout = Layout::build(
        &input,
        Constraints::viewport(Extent {
            width: 80,
            height: 32,
        }),
        &Text,
    )
    .unwrap();
    let caret = layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, domengine::Content::Caret { .. }))
        .unwrap();
    assert_eq!(caret.rect.x, 12);
    assert_eq!(caret.node, None);
    assert_eq!(layout.hit(8, 8), Some(node));
    assert!(document.entry(node).unwrap().children.is_empty());
}
