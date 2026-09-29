use super::{font, tables::*};
use textengine::{Issue, Scalar, Settings, Weight};

#[test]
fn sequence() {
    let font = font(Weight::Normal);
    let [a, b, c, d] = ['A', 'B', 'C', 'D'].map(|character| font.index(character));
    let table = layout(
        b"liga",
        vec![
            lookup(7, 0, extension(1, single(a, c))),
            lookup(4, 0, ligature([c, b], d)),
        ],
    );
    let face = source(b"GSUB", table);
    let shaped = face.shape("AB", Settings::new(100.into(), 0)).unwrap();
    assert_eq!(shaped.glyphs.len(), 1);
    assert_eq!(shaped.glyphs[0].id, d);
    assert_eq!(shaped.clusters[0].source, 0..2);
    assert!(shaped.issues.is_empty());
}
#[test]
fn attachment() {
    let font = font(Weight::Normal);
    let [base, first, second] =
        ['A', '\u{030a}', '\u{0301}'].map(|character| font.index(character));
    let table = layout(
        b"mark",
        vec![
            lookup(
                9,
                0,
                extension(4, mark(base, first, [[400, 700], [100, 200]])),
            ),
            lookup(6, 0, mark(first, second, [[100, 900], [50, 100]])),
        ],
    );
    let face = source(b"GPOS", table);
    let shaped = face
        .shape("A\u{030a}\u{0301}", Settings::new(100.into(), 0))
        .unwrap();
    assert_eq!(shaped.clusters.len(), 1);
    assert_eq!(shaped.clusters[0].source, 0..5);
    assert_eq!(shaped.clusters[0].glyphs, 0..3);
    let advance = Scalar::new(f64::from(face.advance(base).unwrap()) / 20.0);
    assert_eq!(
        shaped.glyphs[1].offset,
        [Scalar::from(15) - advance, (-25).into()]
    );
    assert_eq!(
        shaped.glyphs[2].offset,
        [Scalar::new(17.5) - advance, (-65).into()]
    );
    assert_eq!(shaped.clusters[0].advance, advance);
}
#[test]
fn unsupported() {
    let font = font(Weight::Normal);
    let [a, b] = ['A', 'B'].map(|character| font.index(character));
    for (kind, flags) in [(1, 0x100), (3, 0)] {
        let face = source(
            b"GSUB",
            layout(b"liga", vec![lookup(kind, flags, single(a, b))]),
        );
        let shaped = face.shape("A", Settings::new(20.into(), 0)).unwrap();
        assert_eq!(shaped.glyphs[0].id, a);
        assert_eq!(
            shaped.issues,
            [Issue::Lookup {
                table: *b"GSUB",
                index: 0,
                kind,
                flags
            }]
        );
    }
}
#[test]
fn malformed() {
    let font = font(Weight::Normal);
    let [a, b] = ['A', 'B'].map(|character| font.index(character));
    let settings = Settings::new(20.into(), 0);
    let mut subtable = single(a, b);
    subtable[2..4].copy_from_slice(&0xffffu16.to_be_bytes());
    assert!(
        source(b"GSUB", layout(b"liga", vec![lookup(1, 0, subtable)]))
            .shape("A", settings)
            .is_err()
    );
    let invalid = single(a, font.source().unwrap().count());
    assert!(
        source(b"GSUB", layout(b"liga", vec![lookup(1, 0, invalid)]))
            .shape("A", settings)
            .is_err()
    );
    let mut subtable = mark(a, b, [[0, 0]; 2]);
    subtable[6..8].copy_from_slice(&0u16.to_be_bytes());
    assert!(
        source(b"GPOS", layout(b"mark", vec![lookup(4, 0, subtable)]))
            .shape("AB", settings)
            .is_err()
    );
    let mut subtable = extension(1, single(a, b));
    subtable[4..8].copy_from_slice(&0xffffffffu32.to_be_bytes());
    assert!(
        source(b"GSUB", layout(b"liga", vec![lookup(7, 0, subtable)]))
            .shape("A", settings)
            .is_err()
    );
}

#[test]
fn language() {
    let font = font(Weight::Normal);
    let [a, b] = ['A', 'B'].map(|character| font.index(character));
    let mut table = layout(b"liga", vec![lookup(1, 0, single(a, b))]);
    let mut script = words(&[1]);
    script.extend(b"latn");
    script.extend(words(&[8, 0, 1]));
    script.extend(b"TRK ");
    script.extend(words(&[10, 0, 0xffff, 1, 0]));
    link(&mut table, 4, script);
    let face = source(b"GSUB", table);
    let mut settings = Settings::new(20.into(), 0);
    assert_eq!(face.shape("A", settings).unwrap().glyphs[0].id, a);
    settings.script = *b"latn";
    assert_eq!(face.shape("A", settings).unwrap().glyphs[0].id, a);
    settings.language = Some(*b"TRK ");
    assert_eq!(face.shape("A", settings).unwrap().glyphs[0].id, b);
}
