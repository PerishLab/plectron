use textengine::{Face, Font, Prepared, Selection, Weight};

#[test]
fn intervals() {
    for source in [
        "A\u{030a}ffi e\u{301}\r\nZ",
        "a👩‍👩‍👧‍👦🇨🇳🇺🇸🇦b",
        "क्षि\u{200d}क नमस्ते",
        "سَلَامٌ بَ\u{200d}ت",
    ] {
        for spacing in [0, 3] {
            check(source, spacing);
        }
    }
}

#[test]
fn ligatures() {
    let font = Font {
        face: Face::Toy,
        size: 16.into(),
        weight: Weight::Normal,
    };
    let source = "fffifii";
    let run = font.shape(source, 0).unwrap();
    run.validate(source).unwrap();
    assert_eq!(
        run.glyphs
            .iter()
            .map(|glyph| (glyph.id, glyph.source.clone()))
            .collect::<Vec<_>>(),
        [
            (102, 0..1),
            (102, 1..2),
            (128, 2..4),
            (128, 4..6),
            (105, 6..7)
        ]
    );
    let spaced = font.shape(source, 1).unwrap();
    assert_eq!(spaced.glyphs.len(), source.len());
}

fn check(source: &str, spacing: usize) {
    let ranges = textengine::unicode::graphemes(source);
    let selections = ranges
        .iter()
        .enumerate()
        .map(|(index, range)| Selection {
            source: range.clone(),
            font: Font {
                face: Face::Toy,
                size: (16 + index as i64 % 3).into(),
                weight: Weight::Normal,
            },
            spacing,
        })
        .collect();
    let prepared = Prepared::new(source, selections).unwrap();
    let whole = Font {
        face: Face::Toy,
        size: 16.into(),
        weight: Weight::Normal,
    }
    .prepare(source, spacing)
    .unwrap();
    let boundaries: Vec<_> = std::iter::once(0)
        .chain(ranges.iter().map(|r| r.end))
        .collect();
    for (index, start) in boundaries.iter().enumerate() {
        for end in &boundaries[index..] {
            for prepared in [&prepared, &whole] {
                let run = prepared.shape(*start..*end).unwrap();
                run.validate(&source[*start..*end]).unwrap();
            }
        }
    }
}
