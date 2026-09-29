use ::fixtures::Text;
use domengine::{
    Capture, Constraints, Content, Document, Extent, Input, Layout, Painter, Pipeline, Sheet, html,
    style::Resolver,
};
fn image(width: usize, height: usize, color: [u8; 4]) -> graphics::shared::Raster {
    graphics::shared::Raster::rgba(Extent { width, height }, &color.repeat(width * height)).unwrap()
}
fn rect(layout: &Layout) -> domengine::Rect {
    layout
        .fragments()
        .iter()
        .find_map(|fragment| {
            matches!(fragment.content, Content::Image { .. }).then_some(fragment.rect)
        })
        .unwrap()
}
fn constraints() -> Constraints {
    Constraints::viewport(Extent {
        width: 300,
        height: 200,
    })
}
fn document() -> Document {
    let mut document =
        html::parse("<div><img id='photo' src='one.png'><p>Caption</p></div>").unwrap();
    document
        .image(
            document.find("photo").unwrap(),
            "one.png",
            image(80, 40, [255, 0, 0, 255]),
        )
        .unwrap();
    document
}
#[test]
fn intrinsic() {
    for (css, size) in [
        ("", [80, 40]),
        ("width: 120px;", [120, 60]),
        ("height: 30px;", [60, 30]),
        ("width: 50px; height: 50px;", [50, 50]),
        ("max-width: 40px;", [40, 20]),
        ("max-height: 30px;", [60, 30]),
    ] {
        let document = document();
        let sheet = Sheet::parse(&format!("img {{{css}}}")).unwrap();
        let layout = Layout::build(
            &Input::capture(&document, &sheet).unwrap(),
            constraints(),
            &Text,
        )
        .unwrap();
        let rect = rect(&layout);
        assert_eq!([rect.width, rect.height], size);
        assert_eq!(layout.hit(rect.x + 1, rect.y + 1), document.find("photo"));
    }
}
#[test]
fn lifecycle() {
    let mut document = document();
    let node = document.find("photo").unwrap();
    let old = document.picture(node).unwrap().unwrap().clone();
    let mut capture = Capture::new(Resolver::new(Sheet::default()));
    capture.update(&document, |_| Ok(())).unwrap();
    document.attribute(node, "src", "two.png").unwrap();
    assert!(document.picture(node).unwrap().is_none());
    assert!(capture.update(&document, |_| Ok(())).is_err());
    assert!(!document.image(node, "one.png", old.clone()).unwrap());
    assert!(Input::capture(&document, &Sheet::default()).is_err());
    assert!(document.image(node, "two.png", old).unwrap());
    assert!(document.replace(node, "text").is_err());
    let child = document.text("child").unwrap();
    assert!(document.append(node, child).is_err());
    document
        .attribute(node, "style", "display: inline;")
        .unwrap();
    assert!(Input::capture(&document, &Sheet::default()).is_ok());
}
#[test]
fn rebinding() {
    let mut document = document();
    let node = document.find("photo").unwrap();
    let mut capture = Capture::new(Resolver::new(Sheet::default()));
    let mut pipeline = Pipeline::new(Text);
    let mut painter = Painter::default();
    let first = capture
        .update(&document, |input| pipeline.build(input, constraints()))
        .unwrap();
    let old = painter.build(&first).unwrap();
    document
        .image(node, "one.png", image(80, 40, [0, 0, 255, 128]))
        .unwrap();
    let changed = capture
        .update(&document, |input| pipeline.build(input, constraints()))
        .unwrap();
    assert_eq!(pipeline.inspect().computed, 0);
    assert_eq!(rect(&first), rect(&changed));
    let new = painter.build(&changed).unwrap();
    assert_ne!(
        domengine::raster(
            Extent {
                width: 300,
                height: 200
            },
            &old
        )
        .unwrap(),
        domengine::raster(
            Extent {
                width: 300,
                height: 200
            },
            &new
        )
        .unwrap()
    );
    assert_eq!(
        domengine::raster(
            Extent {
                width: 300,
                height: 200
            },
            &new
        )
        .unwrap(),
        domengine::raster(
            Extent {
                width: 300,
                height: 200
            },
            &changed.paint()
        )
        .unwrap()
    );
    document
        .image(node, "one.png", image(40, 80, [0, 128, 0, 255]))
        .unwrap();
    let changed = capture
        .update(&document, |input| pipeline.build(input, constraints()))
        .unwrap();
    assert!(pipeline.inspect().computed > 0);
    assert_eq!([rect(&changed).width, rect(&changed).height], [40, 80]);
    let fresh = Layout::build(
        &Input::capture(&document, &Sheet::default()).unwrap(),
        constraints(),
        &Text,
    )
    .unwrap();
    assert_eq!(changed.fragments(), fresh.fragments());
}
