use super::*;

#[test]
fn inheritance() {
    let catalog = ::fixtures::catalog();
    for source in [
        "ب\u{200b}ب",
        "ب\u{2060}ب",
        "\u{2067}بب\u{2069}",
        "ب\u{200c}ب",
        "ب\u{200d}ب",
    ] {
        let mixed = catalog
            .shape(source, &request(&["lato", "noto-arabic"]), 0)
            .unwrap();
        let direct = catalog
            .shape(source, &request(&["noto-arabic"]), 0)
            .unwrap();
        mixed.validate(source).unwrap();
        assert_eq!(mixed.spans.len(), 1, "{source}");
        assert_eq!(
            mixed.spans[0].font.face,
            ::fixtures::face("noto-arabic", textengine::Weight::Normal)
        );
        assert_eq!(
            serde_json::to_value(mixed).unwrap(),
            serde_json::to_value(direct).unwrap()
        );
    }
    let source = "\u{200b}\u{2060}";
    let run = catalog
        .shape(source, &request(&["lato", "noto-arabic"]), 0)
        .unwrap();
    run.validate(source).unwrap();
    assert_eq!(
        run.spans[0].font.face,
        ::fixtures::face("lato", textengine::Weight::Normal)
    );
    assert!(run.glyphs.is_empty());
}
