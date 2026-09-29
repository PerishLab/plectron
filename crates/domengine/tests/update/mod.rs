mod isolation;
mod journal;
mod lifecycle;
mod mutations;
mod order;
mod payloads;
mod pipeline;
mod retention;
mod selectors;
use ::fixtures::Text;
use domengine::{
    Capture, Constraints, Document, Extent, Input, Layout, Pipeline, Sheet, html, style::Resolver,
};
use graphics::shared::{Backend, Cpu, Frame, Scene, Visual};
use shared::cache::Manager;

struct Harness {
    document: Document,
    rules: String,
    capture: Capture,
    pipeline: Pipeline<Text>,
    manager: Manager,
}
impl Harness {
    fn new(source: &str, rules: &str) -> Self {
        let manager = Manager::default();
        let config = config::Store::default();
        config.set("layout.impact", true.into());
        Self {
            document: html::parse(source).unwrap(),
            rules: rules.into(),
            capture: Capture::new(
                Resolver::managed(Sheet::parse(rules).unwrap(), &manager).unwrap(),
            ),
            pipeline: Pipeline::configured(Text, &manager, config).unwrap(),
            manager,
        }
    }
    fn check(&mut self) -> Layout {
        let input = Input::capture(&self.document, &Sheet::parse(&self.rules).unwrap()).unwrap();
        let expected = Layout::build(&input, constraints(), &Text).unwrap();
        let actual = self
            .capture
            .update(&self.document, |input| {
                self.pipeline.build(input, constraints())
            })
            .unwrap();
        assert_eq!(actual.revision(), expected.revision());
        assert_eq!(actual.fragments(), expected.fragments());
        assert_eq!(pixels(&actual), pixels(&expected));
        assert!(
            actual.snapshot().impact().unwrap().missed().is_empty(),
            "missed layout impact: {:?}",
            actual.snapshot().impact().unwrap().missed()
        );
        let report = self.capture.inspect();
        assert_eq!(report.committed, Some(self.document.revision()));
        assert_eq!(report.visited + report.skipped, report.nodes);
        assert_eq!(report.rebuilt + report.reused, report.visited);
        for y in (0..128).step_by(8) {
            for x in (0..128).step_by(8) {
                assert_eq!(actual.hit(x, y), expected.hit(x, y));
            }
        }
        actual
    }
}
fn constraints() -> Constraints {
    Constraints::viewport(Extent {
        width: 128,
        height: 128,
    })
}
fn pixels(layout: &Layout) -> graphics::shared::Image {
    let scene = Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap();
    let frame = Frame::new(scene, Visual::new(layout.extent())).unwrap();
    Cpu.render(&frame).unwrap()
}

#[test]
fn locality() {
    let source = format!("<div id='root'>{}</div>", "<p>ABC</p>".repeat(48));
    let mut test = Harness::new(&source, "p { height: 16px; }");
    let original = test.check();
    let before = pixels(&original);
    let root = test.document.find("root").unwrap();
    let paragraph = test.document.entry(root).unwrap().children[0];
    let text = test.document.entry(paragraph).unwrap().children[0];
    test.document.replace(text, "Changed").unwrap();
    test.check();
    let report = test.capture.inspect();
    assert_eq!(report.rebuilt, 4);
    assert_eq!(report.styled, 0);
    assert_eq!(report.skipped, 47);
    assert_eq!(report.changes, 1);
    assert_eq!(pixels(&original), before);
    test.check();
    assert_eq!(test.capture.inspect().visited, 1);
    assert_eq!(test.capture.inspect().rebuilt, 0);
    test.document
        .attribute(root, "style", "background-color: #ff0000;")
        .unwrap();
    test.check();
    assert_eq!(test.capture.inspect().styled, 1);
    assert_eq!(test.capture.inspect().rebuilt, 2);
    assert_eq!(test.pipeline.inspect().retained, 1);
    assert_eq!(test.pipeline.inspect().computed, 0);
    assert_eq!(test.pipeline.inspect().bypassed, 0);
    assert_eq!(test.pipeline.inspect().shaping.preparations, 0);
}

#[test]
fn inheritance() {
    let mut test = Harness::new(
        "<div id='root'><p>A</p><div id='shield'><p>B</p></div></div>",
        "#shield { color: #112233; }",
    );
    test.check();
    let root = test.document.find("root").unwrap();
    test.document
        .attribute(root, "style", "color: #ff0000;")
        .unwrap();
    test.check();
    let report = test.capture.inspect();
    assert_eq!(report.styled, 4);
    assert_eq!(report.rebuilt, 5);
    assert_eq!(report.skipped, 1);
    assert_eq!(test.pipeline.inspect().computed, 0);
    let shield = test.document.find("shield").unwrap();
    test.document.attribute(shield, "id", "unshielded").unwrap();
    test.check();
    assert_eq!(test.capture.inspect().styled, 3);
    test.document.attribute(root, "src", "unused.png").unwrap();
    test.check();
    assert_eq!(test.capture.inspect().rebuilt, 0);
}
