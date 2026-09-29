mod budgets;
mod causality;
mod intrinsic;
mod ownership;
mod paragraphs;
mod recording;
mod retention;
mod shaping;
mod style;
use ::fixtures::Text;
use domengine::{Constraints, Document, Extent, Input, Layout, Pipeline, Sheet, html};
use graphics::shared::{Backend, Cpu, Frame, Scene, Visual};
use shared::cache::Manager;

fn constraints(width: usize) -> Constraints {
    Constraints::viewport(Extent { width, height: 128 })
}

fn eager() -> Pipeline<Text> {
    let config = config::Store::default();
    config.set("layout.recurrence", 0.into());
    Pipeline::configured(Text, &Manager::default(), config).unwrap()
}

fn managed(manager: &Manager) -> Pipeline<Text> {
    let config = config::Store::default();
    config.set("layout.recurrence", 0.into());
    Pipeline::configured(Text, manager, config).unwrap()
}

fn check(document: &Document, pipeline: &mut Pipeline<Text>, width: usize) -> Layout {
    let input = Input::capture(document, &Sheet::default()).unwrap();
    let expected = Layout::build(&input, constraints(width), &Text).unwrap();
    let actual = pipeline.build(&input, constraints(width)).unwrap();
    assert_eq!(actual.revision(), expected.revision());
    assert_eq!(actual.extent(), expected.extent());
    assert_eq!(actual.fragments(), expected.fragments());
    for y in (0..128).step_by(8) {
        for x in (0..width).step_by(8) {
            assert_eq!(actual.hit(x, y), expected.hit(x, y));
        }
    }
    actual
}

fn pixels(layout: &Layout) -> graphics::shared::Image {
    let scene = Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap();
    let frame = Frame::new(scene, Visual::new(layout.extent())).unwrap();
    Cpu.render(&frame).unwrap()
}

#[test]
fn paint() {
    let mut document = html::parse("<div id='box' style='width: 64px; height: 48px; background-color: #112233;'><p>fiAB</p></div>").unwrap();
    let mut pipeline = eager();
    let old = check(&document, &mut pipeline, 128);
    let work = pipeline.inspect().charged;
    let previous = pixels(&old);
    document.attribute(document.find("box").unwrap(), "style", "width: 64px; height: 48px; background-color: #334455; color: #ff0000; opacity: 0.5; translate: 4px 2px;").unwrap();
    let new = check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().hits, 1);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_eq!(pipeline.inspect().charged, work);
    assert_ne!(pixels(&new), previous);
    assert_eq!(pixels(&old), previous);
    assert!(pipeline.inspect().fragments > 0);
}

#[test]
fn locality() {
    let mut document =
        html::parse("<div id='box' style='overflow-wrap: anywhere;'><p id='a'>AA</p><p id='b'>BBBB</p><p id='c'>CC</p></div>")
            .unwrap();
    let mut pipeline = eager();
    let before = check(&document, &mut pipeline, 64);
    document
        .replace(document.find("a").unwrap(), "AAAAAAAAAA")
        .unwrap();
    let after = check(&document, &mut pipeline, 64);
    assert!(pipeline.inspect().hits >= 2);
    assert!(pipeline.inspect().cold > 0);
    let b = document.find("b").unwrap();
    let top = |layout: &Layout| {
        layout
            .fragments()
            .iter()
            .find(|fragment| fragment.node == Some(b.id()))
            .unwrap()
            .rect
            .y
    };
    assert!(top(&after) > top(&before));
    check(&document, &mut pipeline, 96);
    assert!(pipeline.inspect().cold > 0);
    let boxnode = document.find("box").unwrap();
    document.append(boxnode, b).unwrap();
    check(&document, &mut pipeline, 96);
    document.remove(boxnode, b).unwrap();
    check(&document, &mut pipeline, 96);
    document.append(boxnode, b).unwrap();
    check(&document, &mut pipeline, 96);
    document.attribute(b, "style", "display: none;").unwrap();
    check(&document, &mut pipeline, 96);
    document.attribute(b, "style", "width: 20px;").unwrap();
    check(&document, &mut pipeline, 96);
}

#[test]
fn identity() {
    let mut document = html::parse("<p id='p'>AB</p>").unwrap();
    let mut pipeline = eager();
    let before = check(&document, &mut pipeline, 128);
    document.replace(document.find("p").unwrap(), "AB").unwrap();
    let after = check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_ne!(
        before.fragments().last().unwrap().node,
        after.fragments().last().unwrap().node
    );
    let foreign = html::parse("<p id='p'>AB</p>").unwrap();
    check(&foreign, &mut pipeline, 128);
    assert!(pipeline.inspect().reset);
    assert!(pipeline.inspect().computed > 0);
}

#[test]
fn selection() {
    let mut document = html::parse("<input id='input' style='width: 40px;'>").unwrap();
    let node = document.find("input").unwrap();
    document.value(node, "ABCDEF").unwrap();
    let mut pipeline = eager();
    let before = check(&document, &mut pipeline, 128);
    document.focus(Some(node)).unwrap();
    document
        .select(
            node,
            domengine::Selection {
                anchor: 1,
                focus: 6,
            },
        )
        .unwrap();
    let after = check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_ne!(before.fragments(), after.fragments());
    document.caret(node, 0).unwrap();
    check(&document, &mut pipeline, 128);
    assert_eq!(pipeline.inspect().computed, 0);
    document.value(node, "ABCD").unwrap();
    check(&document, &mut pipeline, 128);
    assert!(pipeline.inspect().cold > 0);
}

#[test]
fn flex() {
    let mut document = html::parse("<div id='box' style='display: flex; width: 120px; align-items: stretch;'><p id='a' style='flex-grow: 1;'>AAAA</p><p id='b' style='flex-grow: 2;'>BBBBBB</p></div>").unwrap();
    let mut pipeline = Pipeline::new(Text);
    check(&document, &mut pipeline, 160);
    let operations = pipeline.inspect().operations;
    assert!(operations.flexes > 0);
    assert!(operations.texts > 0);
    assert!(operations.minimums > 0);
    assert!(operations.nodes > 0);
    let work = pipeline.inspect().charged;
    check(&document, &mut pipeline, 160);
    assert_eq!(pipeline.inspect().computed, 0);
    assert_eq!(pipeline.inspect().charged, work);
    for style in [
        "flex-grow: 3;",
        "flex-shrink: 0; min-width: 10px; max-width: 40px;",
        "flex-basis: 30px; align-self: safe flex-end; margin: 2px; padding: 1px;",
    ] {
        document
            .attribute(document.find("a").unwrap(), "style", style)
            .unwrap();
        check(&document, &mut pipeline, 160);
    }
    document.attribute(document.find("box").unwrap(), "style", "display: flex; flex-direction: column; height: 80px; width: 100px; gap: 3px; justify-content: center; overflow: auto;").unwrap();
    check(&document, &mut pipeline, 160);
}

#[test]
fn projection() {
    let document = html::parse(
        "<div style='display: flex; flex-direction: column; width: 96px; height: 64px;'><p>Alpha</p><p>Beta</p></div>",
    )
    .unwrap();
    let mut pipeline = Pipeline::new(Text);
    check(&document, &mut pipeline, 128);
    let requests = pipeline.inspect().flex;
    assert_eq!(requests.base, 2);
    assert_eq!(requests.minimum, 2);
    assert_eq!(requests.projected, 2);
    assert_eq!(requests.resolved, 2);
    assert_eq!(requests.carried, 2);
}

#[test]
fn boundary() {
    let document = html::parse(
        "<div style='display: flex; width: 96px; height: 64px;'><p>Alpha</p><p>Beta</p></div>",
    )
    .unwrap();
    let mut pipeline = Pipeline::new(Text);
    check(&document, &mut pipeline, 128);
    let requests = pipeline.inspect().flex;
    assert_eq!(requests.base, 2);
    assert_eq!(requests.minimum, 2);
    assert_eq!(requests.projected, 0);
    assert_eq!(requests.resolved, 2);
    assert_eq!(requests.carried, 0);
}

#[test]
fn failure() {
    let mut document = html::parse("<div id='box'>AB</div>").unwrap();
    let mut pipeline = Pipeline::new(Text);
    check(&document, &mut pipeline, 128);
    document
        .attribute(document.find("box").unwrap(), "style", "overflow: auto;")
        .unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let expected = Layout::build(&input, constraints(128), &Text).unwrap_err();
    for _ in 0..2 {
        assert_eq!(
            pipeline.build(&input, constraints(128)).unwrap_err(),
            expected
        );
    }
    assert!(pipeline.inspect().failure.is_some());
    document
        .attribute(
            document.find("box").unwrap(),
            "style",
            "height: 32px; overflow: auto;",
        )
        .unwrap();
    check(&document, &mut pipeline, 128);
    assert!(pipeline.inspect().failure.is_none());
}
