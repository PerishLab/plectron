use shared::Resource;
use std::sync::OnceLock;

pub(crate) fn combining(character: char) -> u8 {
    static CLASSES: OnceLock<Vec<u8>> = OnceLock::new();
    CLASSES.get_or_init(|| {
        let mut classes = vec![0; 0x110000];
        for line in Resource::new(include_bytes!(
            "../../resources/unicode/DerivedCombiningClass.txt"
        ))
        .text()
        .expect("Unicode combining data")
        .lines()
        {
            let parts: Vec<_> = line
                .split('#')
                .next()
                .unwrap()
                .split(';')
                .map(str::trim)
                .collect();
            if parts.len() < 2 {
                continue;
            }
            let range: Vec<_> = parts[0]
                .split("..")
                .map(|value| usize::from_str_radix(value, 16).expect("Unicode range"))
                .collect();
            classes[range[0]..=*range.last().unwrap()]
                .fill(parts[1].parse().expect("combining class"));
        }
        classes
    })[character as usize]
}
pub(crate) fn order(character: char) -> u8 {
    let class = combining(character);
    if matches!(
        character,
        '\u{0654}'
            | '\u{0655}'
            | '\u{0658}'
            | '\u{06dc}'
            | '\u{06e3}'
            | '\u{06e7}'
            | '\u{06e8}'
            | '\u{08ca}'
            | '\u{08cb}'
            | '\u{08cd}'
            | '\u{08ce}'
            | '\u{08cf}'
            | '\u{08d3}'
            | '\u{08f3}'
    ) {
        return if class == 220 { 22 } else { 26 };
    }
    match class {
        27..=32 => class + 1,
        33 => 27,
        _ => class,
    }
}
