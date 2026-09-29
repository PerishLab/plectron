mod atoms;
mod fragments;
mod lifecycle;
mod movement;
mod relocation;
mod retention;
mod sequence;
mod typography;
use ::fixtures::Text;
use domengine::{
    Capture, Constraints, Document, Extent, Input, Layout, Painter, Pipeline, Sheet, html,
    style::Resolver,
};
use graphics::shared::{Backend, Cpu, Frame, Ink, Item, Record, Scene, Visual};
use shared::cache::Manager;
use std::sync::Arc;

struct Harness {
    document: Document,
    capture: Capture,
    pipeline: Pipeline<Text>,
    painter: Painter,
    manager: Manager,
}
impl Harness {
    fn new(source: &str) -> Self {
        let manager = Manager::default();
        let config = config::Store::default();
        config.set("layout.recurrence", 0.into());
        Self {
            document: html::parse(source).unwrap(),
            capture: Capture::new(Resolver::managed(Sheet::default(), &manager).unwrap()),
            pipeline: Pipeline::configured(Text, &manager, config).unwrap(),
            painter: Painter::managed(&manager).unwrap(),
            manager,
        }
    }
    fn check(&mut self, visual: Visual) -> Frame {
        let input = Input::capture(&self.document, &Sheet::default()).unwrap();
        let reference = Layout::build(&input, Constraints::viewport(extent()), &Text).unwrap();
        let expected = Frame::new(
            Scene::new(reference.extent(), reference.revision(), reference.paint()).unwrap(),
            visual.clone(),
        )
        .unwrap();
        let (layout, actual) = self
            .capture
            .update(&self.document, |input| {
                let layout = self
                    .pipeline
                    .build(input, Constraints::viewport(extent()))?;
                let items = self.painter.build(&layout)?;
                let scene = Scene::new(layout.extent(), layout.revision(), items)
                    .map_err(|fault| domengine::Fault(fault.to_string()))?;
                let frame = Frame::new(scene, visual)
                    .map_err(|fault| domengine::Fault(fault.to_string()))?;
                Ok((layout, frame))
            })
            .unwrap();
        assert_eq!(Cpu.render(&actual).unwrap(), Cpu.render(&expected).unwrap());
        assert_eq!(layout.fragments(), reference.fragments());
        let spatial = Scene::project(layout.extent(), layout.revision(), layout.spatial()).unwrap();
        assert_eq!(
            actual.scene().mappings(actual.visual()).unwrap(),
            spatial.mappings(actual.visual()).unwrap()
        );
        let mut visited = Vec::new();
        layout.visit(|placement| visited.push(placement.resolve()));
        assert_eq!(visited, reference.fragments());
        for y in (0..128).step_by(8) {
            for x in (0..128).step_by(8) {
                assert_eq!(layout.pick(&actual, x, y), reference.pick(&expected, x, y));
            }
        }
        actual
    }
}
fn extent() -> Extent {
    Extent {
        width: 128,
        height: 128,
    }
}
fn records(items: &[Item]) -> Vec<Arc<Record>> {
    let mut records = Vec::new();
    for item in items {
        match item {
            Item::Draw { record, .. } => records.push(record.clone()),
            Item::Group { items, .. } => records.extend(self::records(items)),
            _ => panic!("retained recording contains a legacy primitive"),
        }
    }
    records
}
fn text(frame: &Frame) -> Arc<Record> {
    records(frame.scene().items()).into_iter().find(|record| {
        matches!(record.ink(), Ink::Text { glyphs, .. } if glyphs[0].id == u16::from(b'A'))
    }).unwrap()
}
#[test]
fn placement() {
    let mut test = Harness::new(
        "<div id='root'><p id='before' style='height: 16px;'>Top</p><button id='button' style='height: 24px; background-color: #224466;'>ABC</button><div id='target'></div></div>",
    );
    let before = test.check(Visual::new(extent()));
    let old = Cpu.render(&before).unwrap();
    let record = text(&before);
    let prefix = test.document.find("before").unwrap();
    test.document
        .attribute(prefix, "style", "height: 40px;")
        .unwrap();
    let moved = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&record, &text(&moved)));
    assert_ne!(old, Cpu.render(&moved).unwrap());
    let button = test.document.find("button").unwrap();
    test.document
        .attribute(button, "style", "height: 24px; background-color: #668822;")
        .unwrap();
    let colored = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&record, &text(&colored)));
    assert_eq!(test.painter.inspect().recorded, 1);
    test.document
        .append(test.document.find("target").unwrap(), button)
        .unwrap();
    let reparented = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&record, &text(&reparented)));
    test.document.replace(button, "ABC").unwrap();
    let replaced = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&record, &text(&replaced)));
    test.document.replace(button, "ABD").unwrap();
    let changed = test.check(Visual::new(extent()));
    assert!(!Arc::ptr_eq(&record, &text(&changed)));
    assert_eq!(Cpu.render(&before).unwrap(), old);
}
#[test]
fn composition() {
    let mut test = Harness::new(
        "<div id='outer' style='width: 80px; height: 48px; overflow: hidden; opacity: 0.5; translate: 4px 2px; background-color: #ff0000;'><div id='scroller' style='height: 32px; overflow: auto;'><p style='height: 32px; background-color: #0000ff;'>ABC</p><p style='height: 32px;'>ABC</p></div></div>",
    );
    let before = test.check(Visual::new(extent()));
    let original = text(&before);
    let node = test.document.find("outer").unwrap();
    test.document.attribute(node, "style", "width: 80px; height: 48px; overflow: hidden; opacity: 0.25; translate: 8px -2px; background-color: #ff0000;").unwrap();
    let shifted = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&original, &text(&shifted)));
    assert_eq!(test.painter.inspect().recorded, 0);
    let mut visual = Visual::new(extent());
    let scroller = test.document.find("scroller").unwrap().id() as u64 * 3 + 2;
    visual.offsets.insert(scroller, 16);
    let scrolled = test.check(visual);
    assert!(Arc::ptr_eq(&original, &text(&scrolled)));
    assert_eq!(test.painter.inspect().recorded, 0);
}
#[test]
fn spatial() {
    let config = config::Store::default();
    config.set("layout.query.spaces", 0.into());
    let mut test = Harness::new(
        "<div style='height: 48px; overflow: auto; translate: 4px 2px;'><button style='height: 24px;'>A</button><button style='height: 24px;'>B</button><button style='height: 24px;'>C</button><button style='height: 24px;'>D</button></div>",
    );
    test.pipeline = Pipeline::configured(Text, &test.manager, config.clone()).unwrap();
    let visual = Visual::new(extent());
    let frame = test.check(visual.clone());
    let layout = test
        .capture
        .update(&test.document, |input| {
            test.pipeline.build(input, Constraints::viewport(extent()))
        })
        .unwrap();
    let projected = Scene::project(layout.extent(), layout.revision(), layout.spatial()).unwrap();
    assert_eq!(
        frame.scene().mappings(&visual).unwrap(),
        projected.mappings(&visual).unwrap()
    );
    config.set("layout.query.spaces", 8.into());
    test.check(visual);
}
#[test]
fn selection() {
    let mut test = Harness::new("<div><input id='editor'><p>ABC</p></div>");
    let editor = test.document.find("editor").unwrap();
    test.document.value(editor, "ABC").unwrap();
    let original = test.check(Visual::new(extent()));
    let record = text(&original);
    test.document.focus(Some(editor)).unwrap();
    test.document
        .select(
            editor,
            domengine::Selection {
                anchor: 0,
                focus: 2,
            },
        )
        .unwrap();
    let selected = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&record, &text(&selected)));
    assert!(test.painter.inspect().recorded > 0);
    test.document.caret(editor, 1).unwrap();
    let caret = test.check(Visual::new(extent()));
    assert!(Arc::ptr_eq(&record, &text(&caret)));
    assert_eq!(test.painter.inspect().recorded, 0);
}

mod pictures;
