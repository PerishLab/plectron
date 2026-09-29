use super::{font, tables::*};
use textengine::{Issue, Scalar, Settings, Weight};

fn substitute(coverage: Vec<u8>, replacements: &[u16]) -> textengine::font::Source {
    let mut subtable = words(&[2, 0, replacements.len() as u16]);
    subtable.extend(words(replacements));
    link(&mut subtable, 2, coverage);
    source(b"GSUB", layout(b"liga", vec![lookup(1, 0, subtable)]))
}

#[test]
fn ranges() {
    let font = font(Weight::Normal);
    let input = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut ids: Vec<_> = input.chars().map(|c| font.index(c)).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut explicit = words(&[1, ids.len() as u16]);
    explicit.extend(words(&ids));
    let mut ranges: Vec<[u16; 3]> = Vec::new();
    for (index, id) in ids.iter().enumerate() {
        if let Some(previous) = ranges
            .last_mut()
            .filter(|r| r[1].checked_add(1) == Some(*id))
        {
            previous[1] = *id;
        } else {
            ranges.push([*id, *id, index as u16]);
        }
    }
    assert!(ranges.len() > 1 && ranges.len() < ids.len());
    let mut compressed = words(&[2, ranges.len() as u16]);
    for range in ranges {
        compressed.extend(words(&range));
    }
    let replacements: Vec<_> = ids.iter().rev().copied().collect();
    let a = substitute(explicit, &replacements);
    let b = substitute(compressed, &replacements);
    let settings = Settings::new(20.into(), 0);
    for text in [input, "", "! .", "aAaAzZ090! ."] {
        let original = a.shape(text, settings).unwrap();
        let actual = b.shape(text, settings).unwrap();
        assert_eq!(
            serde_json::to_value(&actual).unwrap(),
            serde_json::to_value(&original).unwrap()
        );
        let expected: Vec<_> = text
            .chars()
            .map(|c| {
                let id = font.index(c);
                ids.binary_search(&id)
                    .map_or(id, |index| replacements[index])
            })
            .collect();
        assert_eq!(
            actual
                .glyphs
                .iter()
                .map(|glyph| glyph.id)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn limits() {
    let settings = Settings::new(20.into(), 0);
    for coverage in [
        words(&[1, 0]),
        words(&[2, 0]),
        words(&[2, 1, 0, u16::MAX, 0]),
    ] {
        let mut subtable = words(&[1, 0, 0]);
        link(&mut subtable, 2, coverage);
        let face = source(b"GSUB", layout(b"liga", vec![lookup(1, 0, subtable)]));
        let shaped = face.shape("A", settings).unwrap();
        assert_eq!(shaped.glyphs[0].id, font(Weight::Normal).index('A'));
    }
    for coverage in [
        words(&[1, 2, 3]),
        words(&[1, 2, 3, 3]),
        words(&[1, 2, 4, 3]),
        words(&[2, 1, 4, 3, 0]),
        words(&[2, 1, 3, 4, 1]),
        words(&[2, 2, 3, 4, 0, 4, 5, 2]),
        words(&[2, 2, 3, 4, 0, 6, 7, 1]),
        words(&[2, 1, 3, 4]),
        words(&[3, 0]),
    ] {
        let face = substitute(coverage, &[3, 4]);
        for text in ["A", "!", ""] {
            assert!(face.shape(text, settings).is_err());
        }
    }
}

#[test]
fn membership() {
    let font = font(Weight::Normal);
    let [a, b] = ['A', 'B'].map(|c| font.index(c));
    for kind in [4, 6] {
        let face = source(
            b"GPOS",
            layout(b"mark", vec![lookup(kind, 0, mark(a, b, [[0, 0]; 2]))]),
        );
        let settings = Settings::new(20.into(), 0);
        let base = face.shape("A", settings).unwrap();
        if kind == 6 {
            assert_eq!(base.glyphs[0].advance, Scalar::ZERO);
            assert_eq!(base.issues, [Issue::Unpositioned(0..1)]);
        } else {
            assert!(base.glyphs[0].advance > Scalar::ZERO);
            assert!(base.issues.is_empty());
        }
        let orphan = face.shape("B", settings).unwrap();
        assert_eq!(orphan.glyphs[0].advance, Scalar::ZERO);
        assert_eq!(orphan.issues, [Issue::Unpositioned(0..1)]);
        let attached = face.shape("AB", settings).unwrap();
        assert_eq!(attached.clusters.len(), 1);
        assert_eq!(attached.glyphs[1].advance, Scalar::ZERO);
        if kind == 6 {
            assert_eq!(attached.issues, [Issue::Unpositioned(0..2)]);
        } else {
            assert!(attached.issues.is_empty());
        }
    }
}
