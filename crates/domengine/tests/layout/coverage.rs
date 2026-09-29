use ::fixtures::Text;
use domengine::{Constraints, Extent, Input, Layout, Sheet, html};
use graphics::shared::{Frame, Scene, Visual};

#[test]
fn observesdirectflow() {
    let document = html::parse(
        "<div id='timeline'><div class='item'>A</div><div class='item'>B</div><div class='item'>C</div></div>",
    )
    .unwrap();
    let sheet = Sheet::parse(
        "#timeline {width: 200px; height: 100px; overflow: auto;} .item {height: 60px;}",
    )
    .unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let layout = Layout::build(
        &input,
        Constraints::viewport(Extent {
            width: 200,
            height: 100,
        }),
        &Text,
    )
    .unwrap();
    let scene = Scene::project(layout.extent(), layout.revision(), layout.spatial()).unwrap();
    let frame = Frame::new(scene, Visual::new(layout.extent())).unwrap();
    let coverage = layout
        .coverage(&frame, document.find("timeline").unwrap(), 20)
        .unwrap()
        .unwrap();
    assert_eq!(coverage.leading, 0);
    assert_eq!(coverage.trailing, 80);
    assert_eq!(coverage.items, 3);
    assert_eq!(coverage.visible, 0..2);
    assert_eq!(coverage.inset, 0);
}

#[test]
fn requiresscroller() {
    let document = html::parse("<div id='plain'><div>A</div></div>").unwrap();
    let sheet = Sheet::parse("").unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let layout = Layout::build(
        &input,
        Constraints::viewport(Extent {
            width: 200,
            height: 100,
        }),
        &Text,
    )
    .unwrap();
    let scene = Scene::project(layout.extent(), layout.revision(), layout.spatial()).unwrap();
    let frame = Frame::new(scene, Visual::new(layout.extent())).unwrap();
    assert!(
        layout
            .coverage(&frame, document.find("plain").unwrap(), 0)
            .is_err()
    );
}
