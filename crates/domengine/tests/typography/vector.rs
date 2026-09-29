use super::build;
use domengine::{Available, Content};
use graphics::shared::{Font, Scalar};

#[test]
fn placement() {
    let font = Font {
        face: ::fixtures::face("lato", textengine::Weight::Normal),
        size: Scalar::new(23.5),
        ..Font::default()
    };
    let run = font.shape("iiiiiiiiiiiiiiiiiiii", 0).unwrap();
    let width = run
        .clusters
        .iter()
        .fold(Scalar::ZERO, |sum, cluster| sum + cluster.advance);
    let layout = build(
        "iiiiiiiiiiiiiiiiiiii",
        "font-family: lato; font-size: 23.5px;",
        Available::Max,
    );
    let fragment = layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, Content::Text { .. }))
        .unwrap();
    assert_eq!(fragment.rect.width, width.ceil());
    if let Content::Text { glyphs, .. } = &fragment.content {
        assert_eq!(glyphs[19].x - glyphs[0].x, run.clusters[0].advance * 19);
        assert!(glyphs[1].x.value().fract() != 0.0);
    }
}
#[test]
fn identity() {
    let font = Font {
        face: ::fixtures::face("lato", textengine::Weight::Normal),
        ..Font::default()
    };
    let run = font.shape("fi é Å \u{10ffff}", 0).unwrap();
    assert_eq!(run.clusters.len(), 7);
    assert_eq!(run.glyphs[0].id, font.index('ﬁ'));
    assert_eq!(run.clusters[0].source, 0..2);
    assert_eq!(run.clusters[2].source, 3..5);
    assert_eq!(run.glyphs[6].id, 0);
    assert_eq!(run.glyphs[1].id, font.index(' '));
    assert!(
        font.outline(run.glyphs[1].id)
            .unwrap()
            .segments()
            .is_empty()
    );
    assert!(run.clusters[1].advance > Scalar::ZERO);
}
