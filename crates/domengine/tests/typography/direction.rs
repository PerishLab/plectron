use super::*;
use domengine::{Document, Extent, Selection};

fn layout(document: &Document) -> Layout {
    Layout::build(
        &Input::capture(document, &Sheet::default()).unwrap(),
        Constraints::viewport(Extent {
            width: 240,
            height: 100,
        }),
        &Text,
    )
    .unwrap()
}
fn mappings(layout: &Layout) -> Vec<std::ops::Range<usize>> {
    layout
        .fragments()
        .iter()
        .filter_map(|fragment| {
            if let Content::Text { clusters, .. } = &fragment.content {
                Some(clusters.to_vec())
            } else {
                None
            }
        })
        .flatten()
        .collect()
}
#[test]
fn ordering() {
    let text = "abc אבג xyz";
    let layout = build(text, "direction: ltr;", Available::Definite(240));
    let visual: String = mappings(&layout)
        .iter()
        .map(|range| &text[range.clone()])
        .collect();
    assert_eq!(visual, "abc גבא xyz");
    let layout = build("אב גד", "direction: rtl;", Available::Definite(40));
    assert_eq!(mappings(&layout), [2..4, 0..2, 7..9, 5..7]);
    assert_eq!(lines(&layout).len(), 2);
}
#[test]
fn joining() {
    let layout = build(
        "ببب",
        "font-family: noto-arabic; direction: rtl; overflow-wrap: anywhere;",
        Available::Definite(1),
    );
    assert_eq!(lines(&layout).len(), 3);
    let layout = build(
        "ب ب",
        "font-family: noto-arabic; direction: rtl; overflow-wrap: anywhere;",
        Available::Definite(1),
    );
    assert_eq!(lines(&layout).len(), 2);
}
#[test]
fn inheritance() {
    for (html, expected) in [
        ("<div dir='rtl'><p>AB</p></div>", 208),
        ("<div dir='rtl'><p style='direction: ltr;'>AB</p></div>", 0),
        ("<p dir='auto'>אב</p>", 208),
        ("<p dir='auto'>AB</p>", 0),
    ] {
        let document = html::parse(html).unwrap();
        let layout = layout(&document);
        let line = layout
            .fragments()
            .iter()
            .find(|fragment| matches!(fragment.content, Content::Line))
            .unwrap();
        assert_eq!(line.rect.x, expected, "{html}");
    }
}
#[test]
fn selection() {
    let mut document = html::parse("<input id='input' style='width: 240px;'>").unwrap();
    let node = document.find("input").unwrap();
    document.value(node, "abc אבג xyz").unwrap();
    document.focus(Some(node)).unwrap();
    document
        .select(
            node,
            Selection {
                anchor: 2,
                focus: 5,
            },
        )
        .unwrap();
    let layout = layout(&document);
    let ranges: Vec<_> = layout
        .fragments()
        .iter()
        .filter_map(|fragment| {
            matches!(fragment.content, Content::Selection { .. })
                .then_some((fragment.rect.x, fragment.rect.width))
        })
        .collect();
    assert_eq!(ranges, [(32, 32), (96, 16)]);
}
#[test]
fn caret() {
    let mut document = html::parse("<input id='input' dir='rtl' style='width: 240px;'>").unwrap();
    let node = document.find("input").unwrap();
    document.focus(Some(node)).unwrap();
    for (text, offset, expected) in [("", 0, 239), ("אב", 0, 239), ("אב", 2, 207)] {
        document.value(node, text).unwrap();
        document
            .select(
                node,
                Selection {
                    anchor: offset,
                    focus: offset,
                },
            )
            .unwrap();
        let layout = layout(&document);
        let caret = layout
            .fragments()
            .iter()
            .find(|fragment| matches!(fragment.content, Content::Caret { .. }))
            .unwrap();
        assert_eq!(caret.rect.x, expected);
    }
}
