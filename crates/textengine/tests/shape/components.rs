use super::{font, tables::*};
use textengine::{Scalar, Settings, Weight};

fn attachment(base: u16, mark: u16) -> Vec<u8> {
    let marks = words(&[1, 0, 6, 1, 0, 0]);
    let mut components = words(&[3, 0, 0, 0]);
    for (index, [x, y]) in [[100, 200], [300, 400], [500, 600]].into_iter().enumerate() {
        link(&mut components, 2 + index * 2, words(&[1, x, y]));
    }
    let mut bases = words(&[1, 0]);
    link(&mut bases, 2, components);
    let mut data = words(&[1, 0, 0, 1, 0, 0]);
    link(&mut data, 2, words(&[1, 1, mark]));
    link(&mut data, 4, words(&[1, 1, base]));
    link(&mut data, 8, marks);
    link(&mut data, 10, bases);
    data
}

#[test]
fn nested() {
    let font = font(Weight::Normal);
    let [a, b, c, d, e, mark] =
        ['A', 'B', 'C', 'D', 'E', '\u{0301}'].map(|character| font.index(character));
    let substitutions = layout(
        b"liga",
        vec![
            lookup(4, 8, ligature([a, b], d)),
            lookup(4, 8, ligature([d, c], e)),
        ],
    );
    let positions = layout(b"mark", vec![lookup(5, 0, attachment(e, mark))]);
    let mut definition = words(&[1, 0, 0, 0, 0, 0]);
    link(&mut definition, 4, words(&[1, mark, 1, 3]));
    let face = resource(vec![
        (b"GSUB", substitutions),
        (b"GPOS", positions),
        (b"GDEF", definition),
    ]);
    let source = "A\u{0301}BC\u{0301}";
    let shaped = face.shape(source, Settings::new(2000.into(), 0)).unwrap();
    shaped.validate(source).unwrap();
    assert!(shaped.issues.is_empty());
    assert_eq!(shaped.clusters.len(), 1);
    assert_eq!(shaped.clusters[0].source, 0..7);
    assert_eq!(
        shaped
            .glyphs
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>(),
        [e, mark, mark]
    );
    let advance = Scalar::from(i64::from(face.advance(e).unwrap()));
    assert_eq!(
        shaped.glyphs[1].offset,
        [Scalar::from(100) - advance, (-200).into()]
    );
    assert_eq!(
        shaped.glyphs[2].offset,
        [Scalar::from(500) - advance, (-600).into()]
    );
}
