use textengine::bidi::{Bidi, Class, Direction};

#[test]
fn upstream() {
    let data = shared::Resource::new(include_bytes!("../resources/unicode/BidiCharacterTest.txt"))
        .text()
        .unwrap();
    let mut count = 0;
    let mut excluded = 0;
    for line in data.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<_> = line.split(';').map(str::trim).collect();
        let source: String = parts[0]
            .split_whitespace()
            .map(|value| char::from_u32(u32::from_str_radix(value, 16).unwrap()).unwrap())
            .collect();
        if source
            .chars()
            .any(|character| textengine::bidi::class(character) == Class::Legacy)
        {
            excluded += 1;
            continue;
        }
        let direction = match parts[1] {
            "0" => Direction::Ltr,
            "1" => Direction::Rtl,
            _ => Direction::Auto,
        };
        let bidi = Bidi::resolve(&source, direction).unwrap();
        assert_eq!(
            bidi.paragraphs[0].level.to_string(),
            parts[2],
            "base: {line}"
        );
        let levels = bidi.line(0..source.len());
        for ((_, level), expected) in levels.iter().zip(parts[3].split_whitespace()) {
            if expected != "x" {
                assert_eq!(level.to_string(), expected, "levels: {line}");
            }
        }
        let expected: Vec<usize> = parts[4]
            .split_whitespace()
            .map(|value| value.parse().unwrap())
            .collect();
        assert_eq!(bidi.visual(0..source.len()), expected, "order: {line}");
        count += 1;
    }
    assert_eq!((count, excluded), (91_626, 81));
}

#[test]
fn paragraphs() {
    let source = "אב  \nabc  ";
    let bidi = Bidi::resolve(source, Direction::Auto).unwrap();
    assert_eq!(
        bidi.paragraphs
            .iter()
            .map(|part| part.level)
            .collect::<Vec<_>>(),
        [1, 0]
    );
    assert_eq!(
        bidi.line(0..source.len())
            .iter()
            .map(|(_, level)| *level)
            .collect::<Vec<_>>(),
        [1, 1, 1, 1, 1, 0, 0, 0, 0, 0]
    );
    assert_eq!(bidi.visual(0..source.len()), [4, 3, 2, 1, 0, 5, 6, 7, 8, 9]);
    assert_eq!(Bidi::resolve("", Direction::Rtl).unwrap().base(0), 1);
    let isolated = Bidi::resolve("\u{2067}אב\u{2069}abc", Direction::Auto).unwrap();
    assert_eq!(isolated.base(0), 0);
}
#[test]
fn exclusions() {
    for character in ['\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}'] {
        assert!(
            Bidi::resolve(&character.to_string(), Direction::Ltr)
                .unwrap_err()
                .0
                .contains("legacy")
        );
    }
    assert!(Bidi::resolve(&"\u{2067}".repeat(130), Direction::Ltr).is_err());
    assert!(Bidi::resolve(&"a".repeat(65_537), Direction::Ltr).is_err());
}
