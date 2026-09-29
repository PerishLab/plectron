use super::fixtures::{cmap, compound, font, simple};
use textengine::Segment;
use textengine::font::{Problem, Source};

#[test]
fn curves() {
    let glyph = simple(&[
        (0, 0, false),
        (100, 0, false),
        (100, 100, false),
        (0, 100, false),
    ]);
    let source = Source::parse(font(&[vec![], glyph], cmap()).into()).unwrap();
    assert_eq!(source.index('A').unwrap(), Some(1));
    assert_eq!(source.index('B').unwrap(), None);
    assert_eq!(source.advance(1).unwrap(), 500);
    let path = source.outline(1).unwrap();
    assert_eq!(path.segments()[0], Segment::Move([0.into(), 50.into()]));
    assert_eq!(
        path.segments()[1],
        Segment::Quad([0.into(), 0.into()], [50.into(), 0.into()])
    );
    assert_eq!(
        path.segments()
            .iter()
            .filter(|s| matches!(s, Segment::Quad(..)))
            .count(),
        4
    );
}
#[test]
fn components() {
    let glyph = simple(&[(0, 0, true), (100, 0, true), (0, 100, true)]);
    let attached = compound(&[(35, 1, [20, 30]), (1, 1, [1, 0])]);
    let source = Source::parse(font(&[vec![], glyph, attached], cmap()).into()).unwrap();
    let path = source.outline(2).unwrap();
    assert_eq!(path.segments()[0], Segment::Move([20.into(), 30.into()]));
    assert_eq!(path.segments()[4], Segment::Move([120.into(), 30.into()]));
    let cycle = compound(&[(3, 1, [0, 0])]);
    let source = Source::parse(font(&[vec![], cycle], cmap()).into()).unwrap();
    assert_eq!(source.outline(1).unwrap_err(), Problem::Malformed);
}
#[test]
fn matrix() {
    let glyph = simple(&[(0, 0, true), (100, 0, true), (0, 100, true)]);
    let mut rotated = compound(&[(131, 1, [20, 30])]);
    for factor in [0i16, 16384, -16384, 0] {
        rotated.extend(factor.to_be_bytes());
    }
    let source = Source::parse(font(&[vec![], glyph, rotated], cmap()).into()).unwrap();
    let path = source.outline(2).unwrap();
    assert_eq!(path.segments()[1], Segment::Line([20.into(), 130.into()]));
    assert_eq!(path.segments()[2], Segment::Line([(-80).into(), 30.into()]));
}
#[test]
fn supplementary() {
    let mut cmap = vec![0, 0, 0, 1, 0, 3, 0, 10, 0, 0, 0, 12, 0, 12, 0, 0];
    for word in [28u32, 0, 1, 0x1f600, 0x1f600, 1] {
        cmap.extend(word.to_be_bytes());
    }
    let source = Source::parse(font(&[vec![], vec![]], cmap).into()).unwrap();
    assert_eq!(source.index('😀').unwrap(), Some(1));
    assert_eq!(source.index('A').unwrap(), None);
}
#[test]
fn truncated() {
    let glyph = simple(&[(0, 0, true), (100, 0, true), (0, 100, true)]);
    for end in 1..glyph.len() {
        let source = Source::parse(font(&[vec![], glyph[..end].to_vec()], cmap()).into()).unwrap();
        assert!(source.outline(1).is_err(), "truncated at {end}");
    }
}
