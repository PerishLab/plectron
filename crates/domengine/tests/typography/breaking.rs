use super::*;

fn text(layout: &Layout, source: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for fragment in layout.fragments() {
        match &fragment.content {
            Content::Line => lines.push(String::new()),
            Content::Text { clusters, .. } => {
                let mut ranges = clusters.to_vec();
                ranges.dedup();
                lines
                    .last_mut()
                    .unwrap()
                    .extend(ranges.iter().map(|range| &source[range.clone()]));
            }
            _ => (),
        }
    }
    lines
}
#[test]
fn punctuation() {
    for (source, expected) in [
        ("甲乙，丙丁。", vec!["甲", "乙，", "丙", "丁。"]),
        ("甲（乙）丙", vec!["甲", "（乙）", "丙"]),
        ("小さいゃ文字", vec!["小さ", "いゃ", "文字"]),
    ] {
        let layout = build(source, "", Available::Definite(32));
        assert_eq!(text(&layout, source), expected);
    }
}
#[test]
fn controls() {
    for source in ["A\u{00a0}B", "甲\u{2060}乙", "$12,345.67%"] {
        assert_eq!(
            lines(&build(source, "", Available::Definite(16))).len(),
            1,
            "{source}"
        );
    }
    let source = "甲\u{200b}乙";
    let layout = build(source, "", Available::Definite(16));
    assert_eq!(text(&layout, source), ["甲", "乙"]);
    assert_eq!(lines(&layout), [(16, 0), (16, 16)]);
    let source = "A\u{2028}B\u{0085}C";
    assert_eq!(
        text(&build(source, "", Available::Definite(200)), source),
        ["A", "B", "C"]
    );
}
#[test]
fn graphemes() {
    let source = "甲e\u{0301}乙";
    let layout = build(
        source,
        "font-family: lato, noto-sc; font-size: 24px;",
        Available::Definite(24),
    );
    assert_eq!(text(&layout, source), ["甲", "e\u{0301}", "乙"]);
    let layout = build(
        "甲乙丙丁",
        "font-family: noto-sc; font-size: 24px; line-height: 36px;",
        Available::Definite(48),
    );
    assert_eq!(lines(&layout), [(48, 0), (48, 36)]);
}
#[test]
fn whitespace() {
    assert_eq!(lines(&build("A\n)B", "", Available::Definite(16))).len(), 1);
    assert_eq!(lines(&build("A\nB", "", Available::Definite(16))).len(), 2);
    assert_eq!(
        lines(&build(
            "A  B",
            "white-space: pre-wrap;",
            Available::Definite(32)
        )),
        [(48, 0), (16, 16)]
    );
    assert_eq!(
        lines(&build(
            "A\tB",
            "white-space: break-spaces;",
            Available::Definite(128)
        )),
        [(128, 0), (16, 16)]
    );
    assert_eq!(
        lines(&build(
            "甲乙丙丁",
            "white-space: nowrap;",
            Available::Definite(32)
        ))
        .len(),
        1
    );
}
#[test]
fn words() {
    let source = "中文한글AB";
    assert!(lines(&build(source, "", Available::Definite(32))).len() > 1);
    assert_eq!(
        lines(&build(
            source,
            "word-break: keep-all;",
            Available::Definite(32)
        ))
        .len(),
        1
    );
    assert_eq!(build("中文", "", Available::Min).extent().width, 16);
    assert_eq!(
        build("中文", "word-break: keep-all;", Available::Min)
            .extent()
            .width,
        32
    );
    assert_eq!(
        build(
            "中文",
            "word-break: keep-all; overflow-wrap: anywhere;",
            Available::Min
        )
        .extent()
        .width,
        16
    );
    let document = html::parse("<p style='word-break: break-word;'>A</p>").unwrap();
    assert!(Input::capture(&document, &Sheet::default()).is_err());
}

#[test]
fn normalization() {
    let document = html::parse("<p dir='auto'>אב\nA</p>").unwrap();
    let layout = Layout::build(
        &Input::capture(&document, &Sheet::default()).unwrap(),
        Constraints {
            available: [Available::Definite(200), Available::Max],
            known: [None; 2],
        },
        &Text,
    )
    .unwrap();
    assert_eq!(text(&layout, "אב\nA"), ["A\nבא"]);
}

#[test]
fn joining() {
    for source in ["ب\u{200b}ب", "ب\u{200d}ب", "ب\u{2060}ب"] {
        for wrapping in ["normal", "anywhere"] {
            let css =
                format!("font-family: noto-arabic; direction: rtl; overflow-wrap: {wrapping};");
            let count = if wrapping == "anywhere" || source.contains('\u{200b}') {
                2
            } else {
                1
            };
            assert_eq!(
                lines(&build(source, &css, Available::Definite(1))).len(),
                count,
                "{source} {wrapping}"
            );
        }
    }
}
