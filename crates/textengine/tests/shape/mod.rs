mod components;
mod context;
mod coverage;
mod parsing;
mod prepared;
mod ranges;
mod tables;
use textengine::{Font, Issue, Scalar, Settings, Weight};

fn font(weight: Weight) -> Font {
    Font {
        face: ::fixtures::face("lato", weight),
        size: 2000.into(),
        weight,
    }
}
#[test]
fn ligatures() {
    for (weight, advances) in [
        (Weight::Normal, [1145, 512, 1775]),
        (Weight::Bold, [1201, 486, 1849]),
    ] {
        let font = font(weight);
        let run = font.shape("fi ffi", 0).unwrap();
        assert!(run.issues.is_empty());
        assert_eq!(
            run.glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
            [67, 2, 76]
        );
        assert_eq!(
            run.glyphs
                .iter()
                .map(|glyph| glyph.advance)
                .collect::<Vec<_>>(),
            advances.map(Scalar::from)
        );
        assert_eq!(
            run.clusters
                .iter()
                .map(|cluster| cluster.source.clone())
                .collect::<Vec<_>>(),
            [0..2, 2..3, 3..6]
        );
        let spaced = font.shape("fi", 2).unwrap();
        assert_eq!(spaced.glyphs.len(), 2);
        assert_eq!(
            spaced.clusters[0].advance,
            font.advance(font.index('f')).unwrap() + 2.into()
        );
    }
}
#[test]
fn anchors() {
    for (weight, offsets) in [
        (Weight::Normal, [[-96, 0], [-277, -392]]),
        (Weight::Bold, [[-103, 0], [-293, -399]]),
    ] {
        let font = font(weight);
        for ((text, base), offset) in [("e\u{0301}", 'e'), ("A\u{030a}", 'A')]
            .into_iter()
            .zip(offsets)
        {
            let run = font.shape(text, 0).unwrap();
            assert!(run.issues.is_empty());
            assert_eq!(run.clusters.len(), 1);
            assert_eq!(run.clusters[0].source, 0..3);
            assert_eq!(run.clusters[0].glyphs, 0..2);
            assert_eq!(
                run.clusters[0].advance,
                font.advance(font.index(base)).unwrap()
            );
            assert_eq!(run.glyphs[1].advance, Scalar::ZERO);
            assert_eq!(run.glyphs[1].offset, offset.map(Scalar::from));
            let spaced = font.shape(text, 3).unwrap();
            assert_eq!(spaced.clusters.len(), 1);
            assert_eq!(
                spaced.clusters[0].advance,
                run.clusters[0].advance + 3.into()
            );
            assert_eq!(spaced.glyphs[1].offset, run.glyphs[1].offset);
        }
    }
}
#[test]
fn controls() {
    let font = font(Weight::Normal);
    let face = font.source().unwrap();
    let mut settings = Settings::new(font.size, 0);
    settings.ligatures = false;
    assert_eq!(face.shape("ffi", settings).unwrap().glyphs.len(), 3);
    settings.marks = false;
    let nominal = face.shape("A\u{030a}", settings).unwrap();
    assert_eq!(nominal.clusters.len(), 2);
    assert_eq!(nominal.glyphs[1].offset, [Scalar::ZERO; 2]);
    let orphan = font.shape("\u{030a}", 0).unwrap();
    assert_eq!(orphan.issues, [Issue::Unpositioned(0..2)]);
    let missing = font.shape("\u{10ffff}", 0).unwrap();
    assert_eq!(missing.issues, [Issue::Unmapped(0..4)]);
    assert!(face.shape("a", Settings::new((-1).into(), 0)).is_err());
    assert!(font.shape(&"a".repeat(65_537), 0).is_err());
    assert!(font.shape("", 0).unwrap().glyphs.is_empty());
}
#[test]
fn validation() {
    let font = font(Weight::Normal);
    let run = font.shape("A\u{030a}", 0).unwrap();
    let mut invalid = run.clone();
    invalid.clusters[0].glyphs.end = 3;
    assert!(invalid.validate("A\u{030a}").is_err());
    let mut invalid = run.clone();
    invalid.clusters[0].source.end = 2;
    assert!(invalid.validate("A\u{030a}").is_err());
    let mut invalid = run.clone();
    invalid.glyphs[1].id = font.source().unwrap().count();
    assert!(invalid.validate("A\u{030a}").is_err());
    let mut invalid = run;
    invalid.glyphs[1].offset[0] = 9000.into();
    assert!(invalid.validate("A\u{030a}").is_err());
}
