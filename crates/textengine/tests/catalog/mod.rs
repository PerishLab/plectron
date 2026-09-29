use std::sync::Arc;
use textengine::{Catalog, Face, Font, Issue, Request, Shaper, Weight, font::Lease};
mod controls;
mod lifetime;
mod selection;
mod shaping;

fn request(families: &[&str]) -> Request {
    Request {
        families: families.iter().map(|name| (*name).into()).collect(),
        size: 24.into(),
        weight: Weight::Normal,
    }
}
#[test]
fn mixing() {
    let catalog = ::fixtures::catalog();
    for (name, region, source) in [
        ("noto-sc", "noto-sc", "office 中文 e\u{0301}"),
        ("noto-jp", "noto-jp", "office 日本語かなカナ"),
        ("noto-kr", "noto-kr", "office 한국어 한글"),
    ] {
        let run = catalog
            .shape(source, &request(&["missing", "lato", name]), 0)
            .unwrap();
        assert!(run.issues.is_empty(), "{name}: {:?}", run.issues);
        assert!(
            run.spans
                .iter()
                .any(|span| span.font.face == ::fixtures::face("lato", textengine::Weight::Normal))
        );
        assert!(
            run.spans
                .iter()
                .any(|span| span.font.face == ::fixtures::face(region, textengine::Weight::Normal))
        );
        assert!(
            run.clusters
                .iter()
                .any(|cluster| &source[cluster.source.clone()] == "ffi")
        );
        for span in &run.spans {
            for glyph in &run.glyphs[span.glyphs.clone()] {
                assert_ne!(glyph.id, 0);
                span.font.outline(glyph.id).unwrap();
            }
        }
        run.validate(source).unwrap();
    }
}
#[test]
fn clusters() {
    let catalog = ::fixtures::catalog();
    let source = "Ae\u{0301}fi";
    let run = catalog
        .shape(source, &request(&["toy", "lato"]), 0)
        .unwrap();
    assert_eq!(run.spans.len(), 3);
    assert_eq!(
        run.spans[1].font.face,
        ::fixtures::face("lato", textengine::Weight::Normal)
    );
    assert_eq!(
        run.clusters
            .iter()
            .map(|cluster| &source[cluster.source.clone()])
            .collect::<Vec<_>>(),
        ["A", "e\u{0301}", "fi"]
    );
    let spaced = catalog
        .shape("e\u{0301}", &request(&["toy", "lato"]), 3)
        .unwrap();
    let plain = catalog.shape("e\u{0301}", &request(&["lato"]), 0).unwrap();
    assert_eq!(
        spaced.clusters[0].advance,
        plain.clusters[0].advance + 3.into()
    );
    let unsupported = catalog
        .shape("a\u{0301}\u{10ffff}", &request(&["toy"]), 0)
        .unwrap();
    assert_eq!(unsupported.clusters.len(), 2);
    assert!(
        unsupported
            .issues
            .iter()
            .any(|issue| matches!(issue, Issue::Unmapped(_)))
    );
    let regional = catalog
        .shape("\u{1f1e8}\u{1f1f3}", &request(&["lato"]), 0)
        .unwrap();
    assert_eq!(regional.clusters.len(), 1);
}
#[test]
fn identity() {
    let catalog = Catalog::new();
    let first = catalog
        .register(
            "Body",
            Weight::Normal,
            ::fixtures::face("lato", textengine::Weight::Normal),
        )
        .unwrap();
    let second = catalog.register("Body", Weight::Normal, Face::Toy).unwrap();
    let request = request(&["BODY"]);
    assert_eq!(
        catalog.shape("A", &request, 0).unwrap().spans[0].font.face,
        Face::Toy
    );
    assert!(catalog.unregister(&second).unwrap());
    assert_eq!(
        catalog.shape("A", &request, 0).unwrap().spans[0].font.face,
        ::fixtures::face("lato", textengine::Weight::Normal)
    );
    assert!(Catalog::new().unregister(&first).is_err());
    let epoch = catalog.revision();
    assert!(!catalog.unregister(&second).unwrap());
    assert_eq!(catalog.revision(), epoch);
    let mut request = request;
    request.weight = Weight::Bold;
    assert_eq!(
        catalog.shape("A", &request, 0).unwrap().spans[0]
            .font
            .weight,
        Weight::Normal
    );
}
#[test]
fn validation() {
    let catalog = ::fixtures::catalog();
    assert_eq!(
        catalog.shape("A", &request(&["absent"]), 0).unwrap().spans[0]
            .font
            .face,
        Face::Missing
    );
    assert!(catalog.shape("A", &request(&[]), 0).is_err());
    assert!(
        catalog
            .register("bad,name", Weight::Normal, Face::Toy)
            .is_err()
    );
    let mut run = catalog.shape("e\u{0301}", &request(&["lato"]), 0).unwrap();
    run.spans[0].glyphs.end = 1;
    assert!(run.validate("e\u{0301}").is_err());
    run.spans.push(textengine::Span {
        source: 1..3,
        glyphs: 1..2,
        font: Font {
            face: ::fixtures::face("lato", textengine::Weight::Normal),
            ..Font::default()
        },
    });
    assert!(run.validate("e\u{0301}").is_err());
}

#[test]
fn indivisible() {
    let catalog = ::fixtures::catalog();
    let mut run = catalog.shape("e\u{0301}", &request(&["lato"]), 0).unwrap();
    run.clusters[0].source = 0..1;
    run.clusters[0].glyphs = 0..1;
    run.clusters.push(textengine::Cluster {
        source: 1..3,
        glyphs: 1..2,
        advance: 0.into(),
    });
    assert!(run.validate("e\u{0301}").is_err());
}
