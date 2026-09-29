use super::*;
use graphics::shared::Scalar;

#[test]
fn baseline() {
    for source in ["A中B", "HHH中HHH"] {
        alignment(source);
    }
}
fn alignment(source: &str) {
    let layout = build(
        source,
        "font-family: 'Lato', noto-sc; font-size: 24px;",
        Available::Max,
    );
    let mut baseline = None;
    let mut pen = Scalar::ZERO;
    let mut faces = Vec::new();
    for fragment in layout.fragments() {
        if let Content::Text { font, glyphs, .. } = &fragment.content {
            faces.push(font.face.clone());
            for glyph in glyphs.iter() {
                assert_eq!(glyph.x, pen);
                let current = glyph.y + font.ascent();
                assert_eq!(*baseline.get_or_insert(current), current);
                pen = pen + font.advance(glyph.id).unwrap();
            }
        }
    }
    assert_eq!(faces.len(), 3);
    assert_eq!(
        faces[0],
        ::fixtures::face("lato", textengine::Weight::Normal)
    );
    assert_ne!(faces[0], faces[1]);
    assert_eq!(faces[0], faces[2]);
    assert_eq!(lines(&layout)[0].0, pen.ceil());
}
#[test]
fn wrapping() {
    let source = "中e\u{0301}文ffi";
    let layout = build(
        source,
        "font-family: lato, noto-sc; font-size: 24px; overflow-wrap: anywhere;",
        Available::Definite(1),
    );
    assert_eq!(lines(&layout).len(), 6);
    let mut mappings = Vec::new();
    for fragment in layout.fragments() {
        if let Content::Text { clusters, .. } = &fragment.content {
            mappings.extend(clusters.iter().map(|range| &source[range.clone()]));
        }
    }
    assert_eq!(
        mappings,
        ["中", "e\u{0301}", "e\u{0301}", "文", "f", "f", "i"]
    );
}
#[test]
fn selection() {
    let mut document = html::parse(
        "<input id='input' style='font-family: lato, noto-sc; font-size: 24px; width: 200px;'>",
    )
    .unwrap();
    let node = document.find("input").unwrap();
    document.value(node, "A中B").unwrap();
    document.focus(Some(node)).unwrap();
    document
        .select(
            node,
            domengine::Selection {
                anchor: 1,
                focus: 2,
            },
        )
        .unwrap();
    let layout = Layout::build(
        &Input::capture(&document, &Sheet::default()).unwrap(),
        Constraints::viewport(domengine::Extent {
            width: 240,
            height: 80,
        }),
        &Text,
    )
    .unwrap();
    let selection = layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, Content::Selection { .. }))
        .unwrap();
    let caret = layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, Content::Caret { .. }))
        .unwrap();
    let input = layout
        .fragments()
        .iter()
        .find(|fragment| {
            matches!(fragment.content, Content::Box { .. }) && fragment.node == Some(node.id())
        })
        .unwrap();
    assert_eq!(input.rect.height, selection.rect.height + 4);
    assert_eq!(selection.rect.width, 24);
    assert_eq!(caret.rect.x, selection.rect.x + 24);
}

#[test]
fn bounds() {
    for (css, expected) in [("min-height: 40px;", 40), ("max-height: 12px;", 12)] {
        let document = html::parse(&format!("<input id='input' style='{css}'>")).unwrap();
        let node = document.find("input").unwrap();
        let layout = Layout::build(
            &Input::capture(&document, &Sheet::default()).unwrap(),
            Constraints::viewport(domengine::Extent {
                width: 240,
                height: 80,
            }),
            &Text,
        )
        .unwrap();
        let input = layout
            .fragments()
            .iter()
            .find(|fragment| {
                matches!(fragment.content, Content::Box { .. }) && fragment.node == Some(node.id())
            })
            .unwrap();
        assert_eq!(input.rect.height, expected);
    }
}
