mod atoms;
use super::lines;
use ::fixtures::Text;
use domengine::{Constraints, Content, Extent, Input, Layout, Sheet, html};

fn sheet() -> Sheet {
    Sheet::parse("p {font-family: lato, noto-arabic; font-size: 32px; overflow-wrap: anywhere;} .red {color: #bb2200;} .blue {color: #0033bb;} .large {font-size: 48px;}").unwrap()
}
fn render(source: &str, width: usize) -> (domengine::Document, Layout) {
    let document = html::parse(source).unwrap();
    let input = Input::capture(&document, &sheet()).unwrap();
    let layout = Layout::build(&input, constraints(width), &Text).unwrap();
    (document, layout)
}
fn constraints(width: usize) -> Constraints {
    Constraints::viewport(Extent { width, height: 200 })
}
fn pixels(layout: &Layout) -> graphics::shared::Image {
    domengine::raster(
        Extent {
            width: 300,
            height: 200,
        },
        &layout.paint(),
    )
    .unwrap()
}
fn geometry(layout: &Layout) -> Vec<(textengine::Font, graphics::shared::Glyph)> {
    let mut output = Vec::new();
    for fragment in layout.fragments() {
        if let Content::Text { font, glyphs, .. } = &fragment.content {
            let origin = fragment.origin();
            for glyph in glyphs.iter() {
                let value = (
                    font.clone(),
                    graphics::shared::Glyph {
                        id: glyph.id,
                        x: glyph.x + graphics::shared::Scalar::from(origin[0] as i64),
                        y: glyph.y + graphics::shared::Scalar::from(origin[1] as i64),
                    },
                );
                if !output.contains(&value) {
                    output.push(value);
                }
            }
        }
    }
    output.sort_by(|a, b| {
        a.1.y
            .value()
            .total_cmp(&b.1.y.value())
            .then(a.1.x.value().total_cmp(&b.1.x.value()))
            .then(a.1.id.cmp(&b.1.id))
    });
    output
}
#[test]
fn partitions() {
    for (whole, split) in [
        (
            "office affinity",
            "<span>of</span><span>fice</span> <span>aff</span><span>inity</span>",
        ),
        ("بِلَا", "<span>بِ</span><span>ل</span><span>َا</span>"),
        (
            "A\u{030a} ffi",
            "<span>A</span><span>\u{030a}</span> <span>f</span><span>fi</span>",
        ),
        ("A \n B", "<span>A</span> \n <span>B</span>"),
    ] {
        for width in [18, 56, 300] {
            let (_, original) = render(&format!("<p>{whole}</p>"), width);
            let (_, divided) = render(&format!("<p>{split}</p>"), width);
            assert_eq!(lines(&original), lines(&divided), "{whole} at {width}");
            assert_eq!(
                geometry(&original),
                geometry(&divided),
                "{whole} at {width}"
            );
            assert_eq!(pixels(&original), pixels(&divided), "{whole} at {width}");
        }
    }
}
#[test]
fn slices() {
    for (text, markup) in [
        (
            "ffi",
            "<a id='first' class='red'>f</a><a id='last' class='blue'>fi</a>",
        ),
        (
            "لا",
            "<a id='first' class='red'>ل</a><a id='last' class='blue'>ا</a>",
        ),
    ] {
        let (_, original) = render(&format!("<p>{text}</p>"), 300);
        let (document, divided) = render(&format!("<p>{markup}</p>"), 300);
        assert_eq!(geometry(&original), geometry(&divided));
        if text == "ffi" {
            assert_eq!(geometry(&divided).len(), 1);
        }
        let painted: Vec<_> = divided
            .fragments()
            .iter()
            .filter(|fragment| matches!(fragment.content, Content::Text { .. }))
            .collect();
        assert_eq!(painted.len(), 2);
        for id in ["first", "last"] {
            let node = document.find(id).unwrap();
            let leaf = document.entry(node).unwrap().children[0];
            let fragment = painted
                .iter()
                .find(|fragment| fragment.node == Some(leaf.id()))
                .unwrap();
            assert_eq!(
                divided.hit(
                    fragment.rect.x + fragment.rect.width / 2,
                    fragment.rect.y + 10
                ),
                Some(node)
            );
            let Content::Text { clip, clusters, .. } = &fragment.content else {
                unreachable!()
            };
            if text == "ffi" {
                assert!(clip.is_some());
            }
            assert_eq!(clusters[0].start, 0);
        }
        assert_ne!(pixels(&original), pixels(&divided));
        let first = document
            .entry(document.find("first").unwrap())
            .unwrap()
            .children[0];
        let first = painted
            .iter()
            .find(|fragment| fragment.node == Some(first.id()))
            .unwrap();
        let last = document
            .entry(document.find("last").unwrap())
            .unwrap()
            .children[0];
        let last = painted
            .iter()
            .find(|fragment| fragment.node == Some(last.id()))
            .unwrap();
        assert!(if text == "لا" {
            first.rect.x > last.rect.x
        } else {
            first.rect.x < last.rect.x
        });
    }
}
#[test]
fn baseline() {
    let (_, layout) = render(
        "<p>a <strong class='large'>B</strong> <span>c</span></p>",
        300,
    );
    let glyphs = geometry(&layout);
    let baseline = glyphs[0].1.y + glyphs[0].0.ascent();
    assert!(
        glyphs
            .iter()
            .all(|(font, glyph)| glyph.y + font.ascent() == baseline)
    );
    assert!(
        glyphs
            .iter()
            .any(|(font, _)| font.size == 48 && font.weight == textengine::Weight::Bold)
    );
    assert_eq!(lines(&layout).len(), 1);
    let (_, mark) = render(
        "<p><span>A</span><strong class='large'>\u{030a}</strong></p>",
        300,
    );
    let (_, original) = render("<p>A\u{030a}</p>", 300);
    assert_eq!(geometry(&mark), geometry(&original));
}
#[test]
fn flow() {
    let (document, layout) = render(
        "<div><span id='first'>A</span> B<p>C</p><a id='last'>D</a> E</div>",
        300,
    );
    assert_eq!(lines(&layout).len(), 3);
    assert_eq!(layout.hit(2, 5), document.find("first"));
    let last = lines(&layout)[2].1;
    assert_eq!(layout.hit(2, last + 5), document.find("last"));
    for markup in [
        "<p><span style='padding: 2px;'>A</span>B</p>",
        "<p><span><div>A</div></span>B</p>",
    ] {
        let document = html::parse(markup).unwrap();
        assert!(Input::capture(&document, &sheet()).is_err());
    }
}
#[test]
fn rebinding() {
    use domengine::{Capture, Painter, Pipeline, style::Resolver};
    let mut document =
        html::parse("<p><a id='first'>of</a><span id='last'>fice affinity</span></p>").unwrap();
    let mut capture = Capture::new(Resolver::new(sheet()));
    let mut pipeline = Pipeline::new(Text);
    let mut painter = Painter::default();
    let mut previous = None;
    for (step, width) in [80, 80, 80, 20, 300, 80, 80].into_iter().enumerate() {
        let first = document.find("first").unwrap();
        match step {
            1 => document.attribute(first, "class", "red").unwrap(),
            2 => document.attribute(first, "class", "large").unwrap(),
            6 => {
                let last = document.find("last").unwrap();
                document.replace(last, "fice affinity").unwrap();
            }
            _ => {}
        }
        let actual = capture
            .update(&document, |input| pipeline.build(input, constraints(width)))
            .unwrap();
        let expected = Layout::build(
            &Input::capture(&document, &sheet()).unwrap(),
            constraints(width),
            &Text,
        )
        .unwrap();
        assert_eq!(actual.fragments(), expected.fragments());
        let cached = domengine::raster(
            Extent {
                width: 300,
                height: 200,
            },
            &painter.build(&actual).unwrap(),
        )
        .unwrap();
        assert_eq!(cached, pixels(&expected));
        match step {
            1 => {
                assert_eq!(pipeline.inspect().computed, 0);
                assert_ne!(previous.as_ref().unwrap(), &cached);
            }
            2 => assert!(pipeline.inspect().computed > 0),
            _ => {}
        }
        let targets: Vec<_> = actual
            .fragments()
            .iter()
            .filter(|fragment| matches!(fragment.content, Content::Text { .. }))
            .collect();
        for fragment in targets {
            let leaf = document
                .entry(document.find("last").unwrap())
                .unwrap()
                .children[0];
            let wanted = if fragment.node == Some(leaf.id()) {
                document.find("last")
            } else {
                document.find("first")
            };
            if fragment.rect.width > 0 {
                assert_eq!(
                    actual.hit(
                        fragment.rect.x + fragment.rect.width / 2,
                        fragment.rect.y + 10
                    ),
                    wanted
                );
            }
        }
        previous = Some(cached);
    }
}
