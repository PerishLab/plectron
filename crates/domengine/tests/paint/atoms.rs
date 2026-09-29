use ::fixtures::Text;
use domengine::{
    Capture, Constraints, Content, Extent, Input, Layout, Painter, Pipeline, Sheet, html,
    style::Resolver,
};
fn raster(size: [usize; 2], color: [u8; 4]) -> graphics::shared::Raster {
    graphics::shared::Raster::rgba(
        Extent {
            width: size[0],
            height: size[1],
        },
        &color.repeat(size[0] * size[1]),
    )
    .unwrap()
}
fn pixels(layout: &Layout, painter: &mut Painter) -> graphics::shared::Image {
    let extent = Extent {
        width: 240,
        height: 200,
    };
    let image = domengine::raster(extent, &painter.build(layout).unwrap()).unwrap();
    assert_eq!(image, domengine::raster(extent, &layout.paint()).unwrap());
    image
}
#[test]
fn rebinding() {
    let mut document = html::parse("<p>A B CD EF<img id='icon' src='image.png'></p>").unwrap();
    let node = document.find("icon").unwrap();
    let css = "img {display: inline; padding: 2px; margin: 3px; background-color: #ddeeff;} p {width: 96px;}";
    let sheet = Sheet::parse(css).unwrap();
    let constraints = Constraints::viewport(Extent {
        width: 240,
        height: 200,
    });
    let mut capture = Capture::new(Resolver::new(Sheet::parse(css).unwrap()));
    let mut pipeline = Pipeline::new(Text);
    let mut painter = Painter::default();
    let mut previous = None;
    for (index, (size, color)) in [
        ([24, 24], [255, 0, 0, 255]),
        ([24, 24], [0, 0, 255, 128]),
        ([48, 48], [0, 128, 0, 255]),
    ]
    .into_iter()
    .enumerate()
    {
        document
            .image(node, "image.png", raster(size, color))
            .unwrap();
        let layout = capture
            .update(&document, |input| pipeline.build(input, constraints))
            .unwrap();
        match index {
            1 => assert_eq!(pipeline.inspect().computed, 0),
            2 => assert!(pipeline.inspect().computed > 0),
            _ => {}
        }
        let fresh = Layout::build(
            &Input::capture(&document, &sheet).unwrap(),
            constraints,
            &Text,
        )
        .unwrap();
        assert_eq!(layout.fragments(), fresh.fragments());
        let picture = layout
            .fragments()
            .iter()
            .find(|fragment| matches!(fragment.content, Content::Image { .. }))
            .unwrap();
        assert_eq!([picture.rect.width, picture.rect.height], size);
        assert_eq!(
            layout.hit(picture.rect.x + 1, picture.rect.y + 1),
            Some(node)
        );
        let image = pixels(&layout, &mut painter);
        if let Some(previous) = previous {
            assert_ne!(image, previous);
        }
        previous = Some(image);
    }
    let retained = capture
        .update(&document, |input| pipeline.build(input, constraints))
        .unwrap();
    let scene = painter.build(&retained).unwrap();
    let parent = document.entry(node).unwrap().parent.unwrap();
    document.remove(parent, node).unwrap();
    let new = document.element("img").unwrap();
    document.attribute(new, "src", "image.png").unwrap();
    document
        .image(new, "image.png", raster([48, 48], [0, 128, 0, 255]))
        .unwrap();
    document.append(parent, new).unwrap();
    let changed = capture
        .update(&document, |input| pipeline.build(input, constraints))
        .unwrap();
    assert_eq!(pipeline.inspect().computed, 0);
    let picture = changed
        .fragments()
        .iter()
        .find(|fragment| matches!(fragment.content, Content::Image { .. }))
        .unwrap();
    assert_eq!(
        changed.hit(picture.rect.x + 1, picture.rect.y + 1),
        Some(new)
    );
    drop(document);
    drop(painter);
    assert_eq!(
        domengine::raster(
            Extent {
                width: 240,
                height: 200
            },
            &scene
        )
        .unwrap(),
        previous.unwrap()
    );
}
