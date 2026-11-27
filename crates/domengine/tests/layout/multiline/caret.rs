use super::Harness;
use domengine::{Content, Extent, Node, Selection};
use graphics::shared::{Frame, Scene, Visual};

#[test]
fn geometry() {
    let mut multi = Harness::new("width: 120px;", true);
    let node = multi.node;
    multi.document.value(node, "ab\n\ncd\n").unwrap();
    multi
        .document
        .select(
            node,
            Selection {
                anchor: 1,
                focus: 7,
            },
        )
        .unwrap();
    let extent = Extent {
        width: 160,
        height: 120,
    };
    let layout = multi.layout(extent);
    let scene = Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap();
    let _frame = Frame::new(scene, Visual::new(extent)).unwrap();
    let carets: Vec<_> = layout
        .fragments()
        .iter()
        .filter(|fragment| fragment.node == multi.document.cursor(node).map(Node::id))
        .collect();
    assert_eq!(carets.len(), 1);
    let first = layout.caret(node, 0).unwrap();
    let trailing = layout.caret(node, 7).unwrap();
    assert_eq!(trailing.x, first.x);
    assert!(trailing.y >= first.y + first.height * 3);
    assert_eq!(carets[0].rect.y, trailing.y);
    let marks: Vec<_> = layout
        .fragments()
        .iter()
        .filter(|fragment| matches!(fragment.content, Content::Selection { .. }))
        .collect();
    assert_eq!(marks.len(), 3);
    assert!(marks[0].rect.y < marks[1].rect.y && marks[1].rect.y < marks[2].rect.y);
    assert_eq!(layout.locate(node, [first.x, first.y + 2]), Some(0));
    assert_eq!(layout.locate(node, [trailing.x, trailing.y + 2]), Some(7));
    let third = layout.caret(node, 5).unwrap();
    assert_eq!(layout.locate(node, [third.x, third.y + 2]), Some(5));
}

#[test]
fn wrapping() {
    let mut multi = Harness::new("width: 36px; overflow-wrap: anywhere;", true);
    let node = multi.node;
    multi.document.value(node, "abcdef").unwrap();
    let layout = multi.layout(Extent {
        width: 80,
        height: 80,
    });
    let start = layout.caret(node, 0).unwrap();
    let end = layout.caret(node, 6).unwrap();
    assert!(end.y > start.y);
    let down = layout.vertical(node, 1, true).unwrap();
    assert!(down > 1);
    assert_eq!(layout.vertical(node, down, false), Some(1));
    let second = layout.caret(node, down).unwrap();
    assert_eq!(layout.locate(node, [second.x, second.y + 2]), Some(down));
}
