mod breaking;
mod clusters;
mod continuation;
mod direction;
mod metrics;
mod mixing;
mod reshaping;
mod vector;
use ::fixtures::Text;
use domengine::{Available, Constraints, Content, Input, Layout, Sheet, html};

fn build(text: &str, css: &str, width: Available) -> Layout {
    let mut document = html::parse("<p id='text'></p>").unwrap();
    document
        .replace(document.find("text").unwrap(), text)
        .unwrap();
    let sheet = Sheet::parse(&format!("p {{{css}}}")).unwrap();
    Layout::build(
        &Input::capture(&document, &sheet).unwrap(),
        Constraints {
            available: [width, Available::Max],
            known: [None; 2],
        },
        &Text,
    )
    .unwrap()
}
fn lines(layout: &Layout) -> Vec<(usize, usize)> {
    layout
        .fragments()
        .iter()
        .filter_map(|fragment| {
            matches!(fragment.content, Content::Line)
                .then_some((fragment.rect.width, fragment.rect.y))
        })
        .collect()
}
#[test]
fn wrapping() {
    assert_eq!(
        lines(&build("AB CD", "", Available::Definite(64))),
        [(32, 0), (32, 16)]
    );
    assert_eq!(
        lines(&build("ABCDE", "", Available::Definite(32))),
        [(80, 0)]
    );
    assert_eq!(
        lines(&build(
            "ABCDE",
            "overflow-wrap: anywhere;",
            Available::Definite(32)
        )),
        [(32, 0), (32, 16), (16, 32)]
    );
    assert_eq!(
        lines(&build(
            "AB CD",
            "white-space: nowrap; overflow-wrap: anywhere;",
            Available::Definite(32)
        )),
        [(80, 0)]
    );
    assert_eq!(build("ABC D", "", Available::Min).extent().width, 48);
    assert_eq!(
        build("ABC D", "overflow-wrap: anywhere;", Available::Min)
            .extent()
            .width,
        16
    );
    assert_eq!(
        build("ABC D", "white-space: nowrap;", Available::Min)
            .extent()
            .width,
        80
    );
}
#[test]
fn whitespace() {
    assert_eq!(lines(&build("  A \n\t B  ", "", Available::Max)), [(48, 0)]);
    assert_eq!(
        lines(&build(
            "A\r\n\nB\n",
            "white-space: pre;",
            Available::Definite(1)
        )),
        [(16, 0), (0, 16), (16, 32), (0, 48)]
    );
    assert_eq!(
        lines(&build("\tA", "white-space: pre;", Available::Max)),
        [(144, 0)]
    );
    assert_eq!(
        lines(&build("A\tB", "white-space: pre;", Available::Max)),
        [(144, 0)]
    );
    assert!(lines(&build(" \n\t", "", Available::Max)).is_empty());
    assert_eq!(
        lines(&build("A\u{00a0}B", "", Available::Definite(16))),
        [(48, 0)]
    );
    assert_eq!(
        lines(&build(
            "A  B",
            "white-space: break-spaces;",
            Available::Definite(48)
        )),
        [(48, 0), (16, 16)]
    );
    assert_eq!(
        lines(&build(
            "A  B",
            "white-space: break-spaces;",
            Available::Definite(32)
        )),
        [(32, 0), (32, 16)]
    );
    assert_eq!(
        build("A  B", "white-space: break-spaces;", Available::Min)
            .extent()
            .width,
        32
    );
}
#[test]
fn source() {
    let document = html::parse("<p id='text'> A\n  B </p>").unwrap();
    let parent = document.find("text").unwrap();
    let node = document.entry(parent).unwrap().children[0];
    assert!(
        matches!(&document.entry(node).unwrap().kind, domengine::Kind::Text(text) if text == " A\n  B ")
    );
    let layout = build("  fi\r\nA", "white-space: pre;", Available::Max);
    let mapping: Vec<_> = layout
        .fragments()
        .iter()
        .filter_map(|fragment| match &fragment.content {
            Content::Text { clusters, .. } => Some(clusters.to_vec()),
            _ => None,
        })
        .collect();
    assert_eq!(mapping.len(), 2);
    assert_eq!(mapping[0], [0..1, 1..2, 2..4]);
    assert_eq!(mapping[1].len(), 1);
    assert_eq!(mapping[1][0], 6..7);
}

mod inline;
