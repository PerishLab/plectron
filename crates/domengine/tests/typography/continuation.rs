use super::*;

fn glyphs(layout: &Layout) -> Vec<(usize, usize, u16)> {
    let mut result = Vec::new();
    for fragment in layout.fragments() {
        if let Content::Text {
            glyphs, clusters, ..
        } = &fragment.content
        {
            result.extend(
                glyphs
                    .iter()
                    .zip(clusters.iter())
                    .map(|(glyph, source)| (source.start, source.end, glyph.id)),
            );
        }
    }
    result.sort_unstable();
    result
}
#[test]
fn context() {
    let css =
        "font-family: lato, noto-arabic; font-size: 32px; direction: rtl; overflow-wrap: anywhere;";
    for source in [
        "ببب",
        "بِبَبُ",
        "ب\u{200b}ب",
        "ب\u{200d}ب",
        "ب\u{2060}ب",
        "\u{200b}بب\u{200b}",
    ] {
        let whole = build(source, css, Available::Max);
        let narrow = build(source, css, Available::Definite(1));
        assert!(lines(&narrow).len() > 1, "{source}");
        assert!(
            lines(&narrow).iter().all(|(width, _)| *width > 0),
            "{source}"
        );
        assert_eq!(glyphs(&narrow), glyphs(&whole), "{source}");
        let minimum = build(source, css, Available::Min);
        assert_eq!(
            minimum.extent().width,
            lines(&narrow)
                .iter()
                .map(|(width, _)| *width)
                .max()
                .unwrap()
        );
    }
}
#[test]
fn boundaries() {
    let css = "font-family: noto-arabic; direction: rtl; overflow-wrap: anywhere;";
    let font = textengine::Font {
        face: ::fixtures::face("noto-arabic", textengine::Weight::Normal),
        ..textengine::Font::default()
    };
    let isolated = font.shape("ب", 0).unwrap().glyphs[0].id;
    let joined = build("بب", css, Available::Definite(1));
    assert!(glyphs(&joined).iter().all(|(_, _, id)| *id != isolated));
    let hard = build(
        "ب\nب",
        &format!("{css} white-space: pre-wrap;"),
        Available::Definite(1),
    );
    let mut expected = Vec::new();
    for range in [0..2, 3..5] {
        expected.extend(
            font.shape("ب", 0)
                .unwrap()
                .glyphs
                .iter()
                .map(|glyph| (range.start, range.end, glyph.id)),
        );
    }
    expected.sort_unstable();
    assert_eq!(glyphs(&hard), expected);
    assert_eq!(lines(&hard).len(), 2);
    let ligature = build(
        "fi",
        "font-family: lato; overflow-wrap: anywhere;",
        Available::Definite(1),
    );
    assert_eq!(lines(&ligature).len(), 2);
    assert_eq!(glyphs(&ligature)[0].0..glyphs(&ligature)[0].1, 0..1);
}
