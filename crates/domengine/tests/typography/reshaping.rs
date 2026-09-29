use super::*;

fn text(layout: &Layout, source: &str) -> Vec<String> {
    let mut groups: Vec<Vec<std::ops::Range<usize>>> = Vec::new();
    for fragment in layout.fragments() {
        match &fragment.content {
            Content::Line => groups.push(Vec::new()),
            Content::Text { clusters, .. } => groups.last_mut().unwrap().extend(clusters.to_vec()),
            _ => (),
        }
    }
    groups
        .into_iter()
        .map(|mut ranges| {
            ranges.sort_by_key(|range| range.start);
            ranges.dedup();
            ranges.iter().map(|range| &source[range.clone()]).collect()
        })
        .collect()
}
#[test]
fn fitting() {
    let css = "overflow-wrap: anywhere;";
    for (source, width, expected) in [
        ("fi", 24, vec!["fi"]),
        ("fi", 16, vec!["f", "i"]),
        ("Afi", 32, vec!["Af", "i"]),
        ("fi fi", 40, vec!["fi", "fi"]),
        ("\u{200b}ffi\u{200b}", 16, vec!["f", "f", "i"]),
    ] {
        let layout = build(source, css, Available::Definite(width));
        assert_eq!(text(&layout, source), expected, "{source}: {width}");
        assert!(
            lines(&layout)
                .iter()
                .all(|(measured, _)| *measured <= width)
        );
    }
    assert_eq!(
        lines(&build("fi", css, Available::Definite(16))),
        [(16, 0), (16, 16)]
    );
    assert_eq!(build("fi", css, Available::Min).extent().width, 16);
}
#[test]
fn clusters() {
    let css = "font-family: lato, noto-arabic; overflow-wrap: anywhere;";
    for (source, expected) in [
        ("A\u{030a}ffi", vec!["A\u{030a}", "f", "f", "i"]),
        ("بلا", vec!["ب", "ل", "ا"]),
        ("لَا", vec!["لَ", "ا"]),
    ] {
        let layout = build(source, css, Available::Definite(1));
        assert_eq!(text(&layout, source), expected, "{source}");
    }
    let source = "ffi\n\nfi\n";
    let layout = build(
        source,
        &format!("{css} white-space: pre-wrap;"),
        Available::Definite(1),
    );
    assert_eq!(text(&layout, source), ["f", "f", "i", "", "f", "i", ""]);
}
#[test]
fn preservation() {
    let css = "font-family: lato; overflow-wrap: anywhere;";
    for whitespace in ["nowrap", "pre"] {
        let layout = build(
            "ffi",
            &format!("{css} white-space: {whitespace};"),
            Available::Definite(1),
        );
        assert_eq!(text(&layout, "ffi"), ["ffi"]);
    }
    let layout = build("ffi", "font-family: lato;", Available::Definite(1));
    assert_eq!(text(&layout, "ffi"), ["ffi"]);
    let source = "fi 12";
    let layout = build(
        source,
        "direction: rtl; overflow-wrap: anywhere;",
        Available::Definite(32),
    );
    let visual: Vec<String> = layout
        .fragments()
        .iter()
        .filter_map(|fragment| {
            if let Content::Text { clusters, .. } = &fragment.content {
                Some(
                    clusters
                        .iter()
                        .map(|range| &source[range.clone()])
                        .collect(),
                )
            } else {
                None
            }
        })
        .collect();
    assert_eq!(visual, ["fi", "12"]);
}
