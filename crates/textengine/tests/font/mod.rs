use textengine::font::{Problem, Source};
mod fixtures;
mod parsing;
use std::sync::Arc;
use textengine::{Font, Scalar, Segment, Weight};

#[test]
fn repertoire() {
    for weight in [Weight::Normal, Weight::Bold] {
        let font = Font {
            face: ::fixtures::face("lato", weight),
            size: 37.into(),
            weight,
        };
        let source = font.source().unwrap();
        assert_eq!(source.units(), 2000);
        for id in 0..source.count() {
            let outline = source
                .outline(id)
                .unwrap_or_else(|problem| panic!("{weight:?} glyph {id}: {problem:?}"));
            assert!(outline.segments().len() < 65_536);
            source.advance(id).unwrap();
        }
        for character in "ABO8éÅñögyp".chars() {
            let id = source.index(character).unwrap().unwrap();
            assert!(
                source
                    .outline(id)
                    .unwrap()
                    .segments()
                    .iter()
                    .any(|segment| matches!(segment, Segment::Quad(..)))
            );
            assert!(font.advance(id).unwrap() > Scalar::ZERO);
        }
        let space = source.index(' ').unwrap().unwrap();
        assert!(source.outline(space).unwrap().segments().is_empty());
        assert!(source.advance(space).unwrap() > 0);
        assert_eq!(source.index('\u{10ffff}').unwrap(), None);
        assert!(!source.outline(0).unwrap().segments().is_empty());
        assert_eq!(source.advance(source.count()), Err(Problem::Missing));
    }
}

#[test]
fn malformed() {
    let bytes = ::fixtures::bytes("Lato-Regular.ttf");
    for size in [0, 1, 4, 12, 40, 100, 1000, bytes.len() / 2] {
        assert!(Source::parse(Arc::from(&bytes[..size])).is_err());
    }
    let mut broken = bytes.to_vec();
    broken[0..4].copy_from_slice(b"OTTO");
    assert_eq!(
        Source::parse(broken.into()).unwrap_err(),
        Problem::Unsupported
    );
    let mut broken = bytes.to_vec();
    broken[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    assert_eq!(
        Source::parse(broken.into()).unwrap_err(),
        Problem::Malformed
    );
}

#[test]
fn metrics() {
    let font = Font {
        face: ::fixtures::face("lato", textengine::Weight::Normal),
        size: 23.into(),
        ..Font::default()
    };
    let id = font.index('i');
    let advance = font.advance(id).unwrap();
    assert!(advance.value().fract() != 0.0);
    assert_eq!(advance * 100, Scalar::new(advance.value() * 100.0));
    assert!((advance * 100).ceil() < advance.ceil() * 100);
    let large = Font {
        size: 230.into(),
        ..font.clone()
    };
    assert_eq!(font.outline(id).unwrap(), large.outline(id).unwrap());
    assert!((large.advance(id).unwrap().value() - advance.value() * 10.0).abs() < 0.001);
}
