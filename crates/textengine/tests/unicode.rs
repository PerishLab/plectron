use textengine::unicode::graphemes;

#[test]
fn pages() {
    for (source, expected) in [
        ("\u{ff}\u{300}\u{100}", vec![0..4, 4..6]),
        ("\u{10ffff}\u{300}\u{10ffff}", vec![0..6, 6..10]),
        ("\u{ac00}\u{11a8}\u{ac01}", vec![0..6, 6..9]),
        ("\u{1f469}\u{200d}\u{1f680}A", vec![0..11, 11..12]),
        ("\u{1f1e6}\u{1f1e7}\u{1f1e8}", vec![0..8, 8..12]),
    ] {
        assert_eq!(graphemes(source), expected, "{source:?}");
    }
}

#[test]
fn upstream() {
    let data = std::fs::read_to_string(
        std::env::current_dir()
            .unwrap()
            .join("resources/unicode/GraphemeBreakTest.txt"),
    )
    .unwrap();
    let mut count = 0;
    for line in data.lines() {
        let line = line.split('#').next().unwrap().trim();
        if line.is_empty() {
            continue;
        }
        let mut source = String::new();
        let mut boundaries = Vec::new();
        for token in line.split_whitespace() {
            match token {
                "÷" => boundaries.push(source.len()),
                "×" => (),
                _ => source.push(char::from_u32(u32::from_str_radix(token, 16).unwrap()).unwrap()),
            }
        }
        let expected: Vec<_> = boundaries.windows(2).map(|pair| pair[0]..pair[1]).collect();
        assert_eq!(graphemes(&source), expected, "{line}");
        count += 1;
    }
    assert_eq!(count, 766);
}
