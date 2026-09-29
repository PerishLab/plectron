use super::build;
use ::fixtures::Text;
use domengine::{Available, Constraints, Content, Extent, Input, Layout, Selection, Sheet, html};

#[test]
fn sources() {
    let source = "AB 中日 EF";
    let expected: Vec<_> = source
        .char_indices()
        .filter(|(_, character)| *character != ' ')
        .map(|(offset, character)| offset..offset + character.len_utf8())
        .collect();
    for width in [1, 32, 48] {
        let layout = build(
            source,
            "font-family: lato, noto-sc; font-size: 16px;",
            Available::Definite(width),
        );
        assert!(super::lines(&layout).len() > 1);
        let mut ranges: Vec<_> = layout
            .fragments()
            .iter()
            .filter_map(|fragment| match &fragment.content {
                Content::Text { clusters, .. } => Some(clusters.iter().cloned()),
                _ => None,
            })
            .flatten()
            .filter(|range| &source[range.clone()] != " ")
            .collect();
        ranges.sort_by_key(|range| range.start);
        ranges.dedup();
        assert_eq!(ranges, expected);
    }
}

#[test]
fn wrapping() {
    for (source, glyphs) in [("A\u{030a}B", 2), ("e\u{0301}B", 2)] {
        let layout = build(
            source,
            "font-family: lato; font-size: 28px; overflow-wrap: anywhere;",
            Available::Definite(1),
        );
        let lines: Vec<_> = layout
            .fragments()
            .iter()
            .filter_map(|fragment| {
                if let Content::Text {
                    glyphs, clusters, ..
                } = &fragment.content
                {
                    Some((glyphs, clusters, fragment.origin()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].0.len(), glyphs);
        assert_eq!(lines[0].1.len(), glyphs);
        assert!(lines[0].1.iter().all(|range| *range == (0..3)));
        assert_eq!(lines[1].1[0].start, 3);
        assert!(lines[0].0.iter().all(|glyph| glyph.y
            + graphics::shared::Scalar::from(lines[0].2[1] as i64)
            < lines[1].0[0].y + graphics::shared::Scalar::from(lines[1].2[1] as i64)));
    }
}
#[test]
fn editing() {
    let mut document = html::parse(
        "<input id='editor' style='font-family: lato; font-size: 28px; width: 200px;'>",
    )
    .unwrap();
    let node = document.find("editor").unwrap();
    document.value(node, "A\u{030a}").unwrap();
    document.focus(Some(node)).unwrap();
    let mut positions = Vec::new();
    for offset in [0, 1, 2] {
        document
            .select(
                node,
                Selection {
                    anchor: offset,
                    focus: offset,
                },
            )
            .unwrap();
        let layout = Layout::build(
            &Input::capture(&document, &Sheet::default()).unwrap(),
            Constraints::viewport(Extent {
                width: 240,
                height: 80,
            }),
            &Text,
        )
        .unwrap();
        positions.push(
            layout
                .fragments()
                .iter()
                .find(|fragment| matches!(fragment.content, Content::Caret { .. }))
                .unwrap()
                .rect
                .x,
        );
    }
    assert_eq!(positions[0], 0);
    assert!(positions[1] > 0);
    assert_eq!(positions[1], positions[2]);
}
