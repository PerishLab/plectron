use ::fixtures::Text;
use domengine::{Available, Constraints, Content, Document, Extent, Input, Layout, Sheet, html};
use textengine::{Fault, Request, Selection, Shaper};
struct Guard;
impl Shaper for Guard {
    fn revision(&self) -> u64 {
        1
    }
    fn select(
        &self,
        source: &str,
        request: &Request,
        spacing: usize,
    ) -> Result<Vec<Selection>, Fault> {
        assert!(!source.contains('\u{fffc}'), "atom entered font selection");
        Text.select(source, request, spacing)
    }
}
fn document(source: &str) -> Document {
    let mut document = html::parse(&format!("<p>{source}</p>")).unwrap();
    for node in document.walk() {
        if document.entry(node).unwrap().attributes.contains_key("src") {
            document
                .image(
                    node,
                    "icon.png",
                    graphics::shared::Raster::rgba(
                        Extent {
                            width: 24,
                            height: 40,
                        },
                        &[50, 120, 180, 200].repeat(24 * 40),
                    )
                    .unwrap(),
                )
                .unwrap();
        }
    }
    document
}
fn layout(document: &Document, css: &str, width: Available) -> Layout {
    let sheet = Sheet::parse(&format!("img {{display: inline;}} {css}")).unwrap();
    Layout::build(
        &Input::capture(document, &sheet).unwrap(),
        Constraints {
            available: [width, Available::Max],
            known: [None; 2],
        },
        &Guard,
    )
    .unwrap()
}
fn lines(layout: &Layout) -> Vec<domengine::Rect> {
    layout
        .fragments()
        .iter()
        .filter(|fragment| matches!(fragment.content, Content::Line))
        .map(|fragment| fragment.rect)
        .collect()
}
fn image(layout: &Layout) -> &domengine::Fragment {
    layout
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, Content::Image { .. }))
        .unwrap()
}
#[test]
fn placement() {
    let document = document("A<img id='icon' src='icon.png'>B C D");
    let wide = layout(&document, "", Available::Definite(200));
    let picture = image(&wide);
    assert_eq!(
        [
            picture.rect.x,
            picture.rect.y,
            picture.rect.width,
            picture.rect.height
        ],
        [16, 0, 24, 40]
    );
    let text = wide
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, Content::Text { .. }))
        .unwrap();
    if let Content::Text { font, glyphs, .. } = &text.content {
        assert_eq!(
            (glyphs[0].y + font.ascent()).ceil(),
            picture.rect.y + picture.rect.height
        );
    }
    assert_eq!(wide.hit(20, 5), document.find("icon"));
    let narrow = layout(&document, "", Available::Definite(56));
    let rows = lines(&narrow);
    assert!(rows.len() > 1);
    assert!(rows[0].height > rows[1].height);
    assert_eq!(rows[1].height, 16);
    assert_eq!(rows[1].y, rows[0].height);
    let tight = layout(&document, "p {line-height: 0px;}", Available::Definite(200));
    assert_eq!(lines(&tight)[0].height, 40);
    assert_eq!(image(&narrow).rect.width, 24);
}
#[test]
fn wrapping() {
    let document = document("AB<img id='icon' src='icon.png'>CD");
    for (css, expected) in [
        ("", vec![32, 24, 32]),
        ("p {overflow-wrap: anywhere;}", vec![16, 16, 24, 16, 16]),
    ] {
        let actual = layout(&document, css, Available::Definite(16));
        assert_eq!(
            lines(&actual)
                .iter()
                .map(|rect| rect.width)
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            actual
                .fragments()
                .iter()
                .filter(|fragment| matches!(fragment.content, Content::Image { .. }))
                .count(),
            1
        );
    }
    assert_eq!(layout(&document, "", Available::Min).extent().width, 32);
    assert_eq!(
        layout(&document, "p {overflow-wrap: anywhere;}", Available::Min)
            .extent()
            .width,
        24
    );
    assert_eq!(
        lines(&layout(
            &document,
            "p {white-space: nowrap;}",
            Available::Definite(1)
        ))
        .len(),
        1
    );
    let lone = self::document("<img id='icon' src='icon.png'>");
    assert_eq!(lines(&layout(&lone, "", Available::Min))[0].width, 24);
    let tracked = layout(&lone, "p {letter-spacing: 5px;}", Available::Min);
    assert_eq!(lines(&tracked)[0].width, 24);
}
#[test]
fn direction() {
    let document = document("ب<img id='icon' src='icon.png'>ت");
    let layout = layout(
        &document,
        "p {font-family: noto-arabic; font-size: 28px; direction: rtl;}",
        Available::Definite(200),
    );
    let picture = image(&layout);
    let texts: Vec<_> = layout
        .fragments()
        .iter()
        .filter(|fragment| matches!(fragment.content, Content::Text { .. }))
        .collect();
    assert_eq!(texts.len(), 2);
    assert!(texts[0].rect.x < picture.rect.x);
    assert!(texts[1].rect.x >= picture.rect.x + picture.rect.width);
    assert_eq!(
        layout.hit(picture.rect.x + 2, picture.rect.y + 2),
        document.find("icon")
    );
    for fragment in texts {
        let node = document
            .entry(
                document
                    .walk()
                    .into_iter()
                    .find(|node| Some(node.id()) == fragment.node)
                    .unwrap(),
            )
            .unwrap();
        if let (domengine::Kind::Text(source), Content::Text { font, glyphs, .. }) =
            (&node.kind, &fragment.content)
        {
            let run = font.shape(source, 0).unwrap();
            assert_eq!(
                glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>(),
                run.glyphs.iter().map(|glyph| glyph.id).collect::<Vec<_>>()
            );
        }
    }
}
#[test]
fn clusters() {
    for source in [
        "of<span>fi</span>ce<img src='icon.png'>affinity",
        "A<img src='icon.png'>\u{0301}B",
        "<img src='icon.png'><img src='icon.png'>",
        "A\n<img src='icon.png'>\nB\n",
    ] {
        let document = document(source);
        for width in [12, 35, 300] {
            let layout = layout(
                &document,
                "p {font-family: lato; overflow-wrap: anywhere; white-space: pre-wrap;}",
                Available::Definite(width),
            );
            assert!(!lines(&layout).is_empty());
            mappings(&document, &layout);
        }
    }
}

fn mappings(document: &Document, layout: &Layout) {
    for fragment in layout.fragments() {
        let Content::Text { clusters, .. } = &fragment.content else {
            continue;
        };
        let node = document
            .walk()
            .into_iter()
            .find(|node| Some(node.id()) == fragment.node)
            .unwrap();
        if let domengine::Kind::Text(source) = &document.entry(node).unwrap().kind {
            assert!(clusters.iter().all(|range| range.end <= source.len()
                && source.is_char_boundary(range.start)
                && source.is_char_boundary(range.end)));
        }
    }
}
