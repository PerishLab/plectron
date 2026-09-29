use super::{font, tables::*};
use textengine::{Settings, Weight};

fn context(input: &[u16], records: &[[u16; 2]]) -> Vec<u8> {
    let mut data = words(&[3, input.len() as u16, records.len() as u16]);
    data.resize(6 + input.len() * 2, 0);
    for record in records {
        data.extend(words(record));
    }
    for (index, glyph) in input.iter().enumerate() {
        link(&mut data, 6 + index * 2, words(&[1, 1, *glyph]));
    }
    data
}
fn multiple(input: u16, output: &[u16]) -> Vec<u8> {
    let mut data = words(&[1, 0, 1, 0]);
    let mut sequence = words(&[output.len() as u16]);
    sequence.extend(words(output));
    link(&mut data, 6, sequence);
    link(&mut data, 2, words(&[1, 1, input]));
    data
}
fn selected(lookups: Vec<Vec<u8>>) -> Vec<u8> {
    let mut table = layout(b"liga", lookups);
    let feature = u16::from_be_bytes(table[6..8].try_into().unwrap()) as usize;
    table[feature + 10..feature + 12].copy_from_slice(&1u16.to_be_bytes());
    table
}
#[test]
fn identities() {
    let font = font(Weight::Normal);
    let [a, b, c, d, e] = ['A', 'B', 'C', 'D', 'E'].map(|character| font.index(character));
    let table = selected(vec![
        lookup(5, 0, context(&[a, b], &[[0, 1], [1, 2]])),
        lookup(2, 0, multiple(a, &[c, d])),
        lookup(1, 0, single(b, e)),
    ]);
    let face = source(b"GSUB", table);
    let shaped = face.shape("AB", Settings::new(20.into(), 0)).unwrap();
    assert_eq!(
        shaped
            .glyphs
            .iter()
            .map(|glyph| glyph.id)
            .collect::<Vec<_>>(),
        [c, d, e]
    );
    assert_eq!(
        shaped
            .glyphs
            .iter()
            .map(|glyph| glyph.source.clone())
            .collect::<Vec<_>>(),
        [0..1, 0..1, 1..2]
    );
    assert_eq!(shaped.clusters.len(), 2);
    let unmatched = face.shape("AC", Settings::new(20.into(), 0)).unwrap();
    assert_eq!(unmatched.glyphs[0].id, a);
}
#[test]
fn failures() {
    let a = font(Weight::Normal).index('A');
    for records in [vec![[0, 0]], vec![[1, 0]], vec![[0, 99]]] {
        let table = selected(vec![lookup(5, 0, context(&[a], &records))]);
        assert!(
            source(b"GSUB", table)
                .shape("A", Settings::new(20.into(), 0))
                .is_err()
        );
    }
    let table = selected(vec![lookup(5, 0, context(&[], &[]))]);
    assert!(
        source(b"GSUB", table)
            .shape("A", Settings::new(20.into(), 0))
            .is_err()
    );
}

#[test]
fn filtering() {
    let font = font(Weight::Normal);
    let [a, markid] = ['A', '\u{030a}'].map(|character| font.index(character));
    let table = layout(
        b"mark",
        vec![lookup(4, 2, mark(a, markid, [[400, 700], [100, 200]]))],
    );
    let mut definition = words(&[1, 0, 0, 0, 0, 0]);
    link(&mut definition, 4, words(&[1, a, 1, 1]));
    let shaped = resource(vec![(b"GPOS", table), (b"GDEF", definition)])
        .shape("A\u{030a}", Settings::new(20.into(), 0))
        .unwrap();
    assert_eq!(shaped.glyphs[1].offset, [textengine::Scalar::ZERO; 2]);
    assert_eq!(shaped.issues, [textengine::Issue::Unpositioned(1..3)]);
}
