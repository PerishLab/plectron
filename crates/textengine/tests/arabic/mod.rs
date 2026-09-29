use serde::Deserialize;
use textengine::{Font, Weight};

#[derive(Deserialize)]
struct Specimen {
    text: String,
    glyphs: Vec<(u16, i64, i64)>,
    advance: i64,
}
#[test]
fn oracle() {
    let font = Font {
        face: ::fixtures::face("noto-arabic", textengine::Weight::Normal),
        size: 1000.into(),
        weight: Weight::Normal,
    };
    let fixtures: Vec<Specimen> = serde_json::from_str(include_str!("oracle.json")).unwrap();
    for mut specimen in fixtures {
        let run = font.shape(&specimen.text, 0).unwrap();
        assert!(run.issues.is_empty(), "{}: {:?}", specimen.text, run.issues);
        let mut pen = 0.0;
        let mut glyphs = Vec::new();
        for cluster in run.clusters.iter().rev() {
            let mut x = pen;
            for glyph in &run.glyphs[cluster.glyphs.clone()] {
                glyphs.push((
                    glyph.id,
                    (x + glyph.offset[0].value()).round() as i64,
                    glyph.offset[1].value().round() as i64,
                ));
                x += glyph.advance.value();
            }
            pen += cluster.advance.value();
        }
        glyphs.sort_unstable();
        specimen.glyphs.sort_unstable();
        assert_eq!(glyphs, specimen.glyphs, "{}", specimen.text);
        assert_eq!(pen, specimen.advance as f64, "{}", specimen.text);
    }
}

#[test]
fn controls() {
    let font = Font {
        face: ::fixtures::face("noto-arabic", textengine::Weight::Normal),
        size: 32.into(),
        weight: Weight::Normal,
    };
    let source = "\u{2067}\u{200e}\u{061c}\u{2069}";
    let run = font.shape(source, 3).unwrap();
    run.validate(source).unwrap();
    assert!(run.issues.is_empty());
    assert!(run.glyphs.is_empty());
    assert!(run.clusters.iter().all(|cluster| cluster.advance == 0));
    assert_eq!(run.spans[0].source, 0..source.len());
    let source = "(سلام)";
    let mut run = font.shape(source, 0).unwrap();
    let bidi = textengine::bidi::Bidi::resolve(source, textengine::bidi::Direction::Rtl).unwrap();
    run.orient(source, &bidi).unwrap();
    assert_eq!(run.glyphs[0].id, font.index(')'));
    assert_eq!(
        run.glyphs[0].advance,
        font.advance(font.index(')')).unwrap()
    );
    assert_eq!(run.glyphs.last().unwrap().id, font.index('('));
    assert_eq!(run.clusters[0].source, 0..1);
}
