use super::{Available, Content, Input, Layout, Sheet, Text, build, html, lines};
use domengine::{Constraints, Extent, Selection};
use graphics::shared::{Font, Weight};

#[test]
fn geometry() {
    let layout = build(
        "AB CD",
        "font-size: 24px; line-height: 2; text-align: center;",
        Available::Definite(100),
    );
    assert_eq!(lines(&layout), [(48, 0), (48, 48)]);
    let text: Vec<_> = layout
        .fragments()
        .iter()
        .filter_map(|fragment| match &fragment.content {
            Content::Text { font, glyphs, .. } => Some((fragment.rect, font, glyphs)),
            _ => None,
        })
        .collect();
    assert_eq!(text[0].0.x, 26);
    assert_eq!(text[0].0.height, 48);
    assert_eq!(text[0].1.size, 24);
    assert_eq!(text[0].2[0].y, 12);
    assert_eq!(
        text[1].2[0].y + graphics::shared::Scalar::from(text[1].0.y as i64),
        60
    );
    let tight = build("A", "font-size: 20px; line-height: 10px;", Available::Max);
    let glyph = tight
        .fragments()
        .iter()
        .find_map(|fragment| match &fragment.content {
            Content::Text { glyphs, .. } => glyphs.first(),
            _ => None,
        })
        .unwrap();
    assert_eq!(glyph.y, -5);
    assert_eq!(lines(&tight), [(20, 0)]);
    assert_eq!(
        lines(&build("fi", "letter-spacing: 2px;", Available::Max)),
        [(34, 0)]
    );
    let zero = build("AB", "font-size: 0;", Available::Max);
    assert_eq!(lines(&zero), [(0, 0)]);
}
#[test]
fn inheritance() {
    let document = html::parse("<div><p>AB</p><input id='input'></div>").unwrap();
    for (leading, height) in [("1.5", 12), ("30px", 30)] {
        let sheet = Sheet::parse(&format!("div {{font-size: 32px; font-weight: bold; line-height: {leading}; letter-spacing: 2px; text-align: end;}} p {{font-size: 8px;}}")).unwrap();
        let layout = Layout::build(
            &Input::capture(&document, &sheet).unwrap(),
            Constraints::viewport(Extent {
                width: 100,
                height: 100,
            }),
            &Text,
        )
        .unwrap();
        let fragment = layout
            .fragments()
            .iter()
            .find(|fragment| matches!(fragment.content, Content::Text { .. }))
            .unwrap();
        assert_eq!(fragment.rect.height, height);
        assert_eq!(fragment.rect.x, 82);
        assert!(matches!(
            fragment.content,
            Content::Text {
                font: Font {
                    size,
                    weight: Weight::Bold,
                    ..
                },
                ..
            } if size == 8
        ));
        let input = document.find("input").unwrap().id();
        let rect = layout
            .fragments()
            .iter()
            .find(|fragment| fragment.node == Some(input))
            .unwrap()
            .rect;
        assert_eq!(rect.height, if leading == "1.5" { 52 } else { 34 });
    }
}
#[test]
fn editing() {
    let mut document = html::parse("<input id='input' style='font-size: 24px; line-height: 2; letter-spacing: 2px; width: 200px;'>").unwrap();
    let node = document.find("input").unwrap();
    document.value(node, "fiA").unwrap();
    document.focus(Some(node)).unwrap();
    document
        .select(
            node,
            Selection {
                anchor: 1,
                focus: 3,
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
    assert_eq!(
        (
            selection.rect.x,
            selection.rect.width,
            selection.rect.height
        ),
        (26, 50, 48)
    );
    assert_eq!(caret.rect.x, 76);
    let glyphs = layout
        .fragments()
        .iter()
        .find_map(|fragment| match &fragment.content {
            Content::Text { glyphs, .. } => Some(glyphs),
            _ => None,
        })
        .unwrap();
    assert_eq!(glyphs.len(), 3);
    assert_eq!((glyphs[1].x, glyphs[1].y), (26.into(), 12.into()));
}
#[test]
fn rejection() {
    for css in [
        "font-size: -1px;",
        "font-size: 4097px;",
        "font-weight: 500;",
        "line-height: -1;",
        "line-height: NaN;",
        "letter-spacing: -1px;",
        "text-align: justify;",
    ] {
        assert!(Sheet::parse(&format!("p {{{css}}}")).is_err(), "{css}");
    }
}
