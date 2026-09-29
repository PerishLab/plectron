use textengine::breaking::{Kind, opportunities};

#[test]
fn upstream() {
    let source = shared::Resource::new(include_bytes!("../resources/unicode/LineBreakTest.txt"))
        .text()
        .unwrap();
    let mut count = 0;
    let mut failures = Vec::new();
    for line in source.lines() {
        let sample = line.split('#').next().unwrap().trim();
        if sample.is_empty() {
            continue;
        }
        let mut text = String::new();
        let mut expected = Vec::new();
        for part in sample.split_whitespace() {
            match part {
                "÷" => expected.push(text.len()),
                "×" => (),
                code => text.push(char::from_u32(u32::from_str_radix(code, 16).unwrap()).unwrap()),
            }
        }
        let actual: Vec<_> = opportunities(&text)
            .unwrap()
            .iter()
            .map(|entry| entry.offset)
            .collect();
        if actual != expected {
            failures.push(format!("{line}\nactual={actual:?} expected={expected:?}"));
        }
        count += 1;
    }
    assert!(
        failures.is_empty(),
        "{} of {count} failed:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert_eq!(count, 19_338);
}
#[test]
fn boundaries() {
    assert_eq!(opportunities("").unwrap()[0].kind, Kind::Hard);
    assert!(opportunities(&"a".repeat(65_537)).is_err());
    let points = opportunities("A\r\nB\u{2028}C").unwrap();
    assert_eq!(
        points
            .iter()
            .map(|point| (point.offset, point.kind))
            .collect::<Vec<_>>(),
        [(3, Kind::Hard), (7, Kind::Hard), (8, Kind::Hard)]
    );
}
