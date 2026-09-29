use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Specimen {
    text: String,
    face: String,
    span: [usize; 2],
    glyphs: Vec<(u16, i64, i64)>,
    advance: i64,
}
#[test]
fn oracle() {
    let fixtures: Vec<Specimen> = serde_json::from_str(include_str!("ranges.json")).unwrap();
    for mut specimen in fixtures {
        let arabic = specimen.face == "Arabic";
        let font = Font {
            face: if arabic {
                ::fixtures::face("noto-arabic", textengine::Weight::Normal)
            } else {
                ::fixtures::face("lato", textengine::Weight::Normal)
            },
            size: if arabic { 1000.into() } else { 2000.into() },
            weight: Weight::Normal,
        };
        let prepared = font.prepare(&specimen.text, 0).unwrap();
        let range = specimen.span[0]..specimen.span[1];
        let run = prepared.shape(range.clone()).unwrap();
        run.validate(&specimen.text[range.clone()]).unwrap();
        assert!(
            run.issues.is_empty(),
            "{} {:?}: {:?}",
            specimen.text,
            range,
            run.issues
        );
        let mut order: Vec<_> = run.clusters.iter().collect();
        if arabic {
            order.reverse();
        }
        let mut pen = Scalar::ZERO;
        let mut glyphs = Vec::new();
        for cluster in order {
            let mut x = pen;
            for glyph in &run.glyphs[cluster.glyphs.clone()] {
                glyphs.push((
                    glyph.id,
                    (x + glyph.offset[0]).value().round() as i64,
                    glyph.offset[1].value().round() as i64,
                ));
                x = x + glyph.advance;
            }
            pen = pen + cluster.advance;
        }
        glyphs.sort_unstable();
        specimen.glyphs.sort_unstable();
        assert_eq!(glyphs, specimen.glyphs, "{} {:?}", specimen.text, range);
        assert_eq!(
            pen.value(),
            specimen.advance as f64,
            "{} {:?}",
            specimen.text,
            range
        );
    }
}
#[test]
fn boundaries() {
    let font = font(Weight::Normal);
    let source = "A\u{030a}ffi";
    let run = font.shape(source, 0).unwrap();
    let prepared = font.prepare(source, 0).unwrap();
    for range in [0..1, 1..3, 2..3, 6..7, usize::MAX..usize::MAX] {
        assert!(prepared.shape(range).is_err());
    }
    for range in [0..0, 3..3, 6..6] {
        let run = prepared.shape(range).unwrap();
        run.validate("").unwrap();
        assert!(run.glyphs.is_empty());
    }
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        serde_json::to_value(prepared.shape(0..source.len()).unwrap()).unwrap()
    );
    assert!(font.prepare(source, 4097).is_err());
    let bidi = textengine::bidi::Bidi::resolve("ب(", textengine::bidi::Direction::Rtl).unwrap();
    let mut run = font.shape("(", 0).unwrap();
    assert!(run.project("(", &bidi, usize::MAX).is_err());
    run.project("(", &bidi, 2).unwrap();
    assert_eq!(run.glyphs[0].id, font.index(')'));
}
#[test]
fn fallback() {
    use textengine::{Request, Shaper};
    let catalog = ::fixtures::catalog();
    for source in ["中ffiبلا", "\u{200b}بلا", "fi\u{10ffff}"] {
        let request = Request {
            families: vec!["lato".into(), "noto-sc".into(), "noto-arabic".into()].into(),
            ..Request::default()
        };
        let run = catalog.shape(source, &request, 0).unwrap();
        let prepared = catalog.prepare(source, &request, 0).unwrap();
        assert_eq!(
            serde_json::to_value(&run).unwrap(),
            serde_json::to_value(prepared.shape(0..source.len()).unwrap()).unwrap()
        );
        for range in textengine::unicode::graphemes(source) {
            prepared
                .shape(range.clone())
                .unwrap()
                .validate(&source[range])
                .unwrap();
        }
    }
}

#[test]
fn selections() {
    use textengine::{Prepared, Selection};
    let font = font(Weight::Normal);
    let source = "ffi";
    let selected = |range| Selection {
        source: range,
        font: font.clone(),
        spacing: 0,
    };
    let prepared = Prepared::new(source, vec![selected(0..1), selected(1..3)]).unwrap();
    let run = prepared.shape(0..3).unwrap();
    assert_eq!(run.spans.len(), 1);
    assert_eq!(
        serde_json::to_value(&run).unwrap(),
        serde_json::to_value(font.shape(source, 0).unwrap()).unwrap()
    );
    for spans in [
        vec![],
        vec![selected(0..2)],
        vec![selected(1..3)],
        vec![selected(0..2), selected(1..3)],
        vec![selected(0..4)],
    ] {
        assert!(Prepared::new(source, spans).is_err());
    }
    assert!(Prepared::new("A\u{030a}", vec![selected(0..1), selected(1..3)]).is_err());
}
#[test]
fn context() {
    use textengine::{Prepared, Selection};
    let source = "ببب";
    let font = Font {
        face: ::fixtures::face("noto-arabic", textengine::Weight::Normal),
        size: 32.into(),
        weight: Weight::Normal,
    };
    let selected = source
        .char_indices()
        .enumerate()
        .map(|(index, (start, character))| Selection {
            source: start..start + character.len_utf8(),
            font: Font {
                size: (32 + index as i64).into(),
                ..font.clone()
            },
            spacing: 0,
        })
        .collect();
    let run = Prepared::new(source, selected)
        .unwrap()
        .shape(0..source.len())
        .unwrap();
    let expected = font.shape(source, 0).unwrap();
    assert_eq!(
        run.glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
        expected
            .glyphs
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(run.spans.len(), 3);
}

#[test]
fn projection() {
    use textengine::bidi::{Bidi, Direction};
    let font = font(Weight::Normal);
    for text in [
        "",
        "(A\u{030a}) ffi [12]",
        "אב (ffi) [12]",
        "\u{2067}(A)\u{2069}",
    ] {
        for direction in [Direction::Ltr, Direction::Rtl, Direction::Auto] {
            let bidi = Bidi::resolve(text, direction).unwrap();
            let mut run = font.shape(text, 0).unwrap();
            let sources: Vec<_> = run
                .glyphs
                .iter()
                .map(|glyph| glyph.source.clone())
                .collect();
            run.orient(text, &bidi).unwrap();
            run.validate(text).unwrap();
            assert_eq!(
                run.glyphs
                    .iter()
                    .map(|glyph| glyph.source.clone())
                    .collect::<Vec<_>>(),
                sources
            );
        }
    }
    let bidi = Bidi::resolve("(", Direction::Rtl).unwrap();
    let mut invalid = font.shape("(", 0).unwrap();
    invalid.clusters[0].source.end = 0;
    let original = serde_json::to_value(&invalid).unwrap();
    assert!(invalid.orient("(", &bidi).is_err());
    assert_eq!(serde_json::to_value(&invalid).unwrap(), original);
    let mut overflow = font.shape("(", 0).unwrap();
    overflow.clusters[0].advance = 8192.into();
    overflow.glyphs[0].advance = Scalar::ZERO;
    overflow.validate("(").unwrap();
    assert_eq!(
        overflow.orient("(", &bidi).unwrap_err().0,
        "invalid cluster advance"
    );
    assert!(overflow.validate("(").is_err());
}
