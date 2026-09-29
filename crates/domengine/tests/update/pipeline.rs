use super::Harness;
use domengine::{
    Capture, Constraints, Document, Extent, Input, Layout, Pipeline, Sheet, html, style::Resolver,
};
use fixtures::Text;
use shared::cache::{Command, Manager};

#[test]
fn measurement() {
    let document = html::parse("<div><p>Hello</p><p>World</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let constraints = Constraints::viewport(Extent {
        width: 128,
        height: 128,
    });
    let expected = Layout::build(&input, constraints, &Text).unwrap();
    let config = config::Store::default();
    let mut engine =
        Pipeline::configured(Text, &shared::cache::Manager::default(), config.clone()).unwrap();
    let mut trace = pipeline::Trace::default();
    for retained in [0, 1] {
        let layout = engine.observe(&input, constraints, &mut trace).unwrap();
        assert_eq!(layout.fragments(), expected.fragments());
        assert_eq!(engine.inspect().retained, retained);
        assert_eq!(trace.spans.len(), 2);
        assert_eq!(trace.spans[0].name, "layout.measurement");
        assert_eq!(trace.spans[1].name, "layout.fragments");
        assert!(trace.spans.iter().all(|span| span.success));
        assert_eq!(trace.topology.as_ref().unwrap().vertices()[1].incoming, [0]);
    }
    let invalid = Constraints::viewport(Extent {
        width: 1_048_577,
        height: 128,
    });
    assert!(engine.observe(&input, invalid, &mut trace).is_err());
    assert_eq!(trace.spans.len(), 1);
    assert!(!trace.spans[0].success);
    assert_eq!(engine.inspect().assembly, 0);
    assert_eq!(
        engine.build(&input, constraints).unwrap().fragments(),
        expected.fragments()
    );
    config.set("layout.retention", 0.into());
    assert!(engine.observe(&input, constraints, &mut trace).is_err());
    assert!(trace.spans.is_empty());
    assert!(trace.topology.is_none());
}

#[test]
fn workset() {
    let rows = (0..140)
        .map(|index| format!("<p id='row{index}'>row {index}</p>"))
        .collect::<String>();
    let mut document = html::parse(&format!("<div>{rows}</div>")).unwrap();
    let manager = Manager::default();
    let sheet = Sheet::default();
    let mut capture = Capture::new(Resolver::managed(sheet.clone(), &manager).unwrap());
    let mut engine = Pipeline::managed(Text, &manager).unwrap();
    let constraints = Constraints::viewport(Extent {
        width: 128,
        height: 128,
    });
    capture
        .update(&document, |input| engine.build(input, constraints))
        .unwrap();
    for cache in manager.inspect() {
        manager.command(cache.id, Command::Clear).unwrap();
    }
    let last = document.find("row139").unwrap();
    let text = document.entry(last).unwrap().children[0];
    document.replace(text, "Changed").unwrap();
    let actual = capture
        .update(&document, |input| engine.build(input, constraints))
        .unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let expected = Layout::build(&input, constraints, &Text).unwrap();
    assert_eq!(actual.fragments(), expected.fragments());
    assert!(engine.inspect().planned > 0);
    assert!(actual.snapshot().impact().is_none());
    let collection = actual.snapshot().collection();
    assert_eq!(
        collection.visited + collection.reused,
        actual.tree().nodes().len()
    );
    assert!(collection.visited < actual.tree().nodes().len());
    assert!(collection.reused > 0);
    assert!(collection.shared > 0);
    assert!(collection.shared < collection.pages);
    assert!(actual.snapshot().delta().survey().compared < actual.tree().nodes().len());
    assert_eq!(actual.snapshot().delta().survey().scanned, 0);
    let narrow = Constraints::viewport(Extent {
        width: 96,
        height: 128,
    });
    let actual = capture
        .update(&document, |input| engine.build(input, narrow))
        .unwrap();
    let expected = Layout::build(&input, narrow, &Text).unwrap();
    assert_eq!(actual.fragments(), expected.fragments());
    assert_eq!(engine.inspect().planned, 0);
}

#[test]
fn impactwork() {
    let mut test = Harness::new(
        "<div id='root'><p id='first'>A</p><p id='second'>B</p></div>",
        "",
    );
    let first = test.check();
    let branches = first.delta().added().len();
    let idle = test.check();
    assert_eq!(idle.snapshot().collection().visited, 0);
    assert_eq!(
        idle.snapshot().collection().reused,
        idle.snapshot().states().count()
    );
    let survey = idle.snapshot().impact().unwrap().survey();
    assert_eq!(survey.compared, idle.tree().nodes().len());
    assert_eq!(survey.topology, idle.tree().edges().len());
    assert_eq!(survey.indexed, 0);
    assert_eq!(survey.routed, 0);
    let survey = idle.snapshot().delta().survey();
    assert_eq!(survey.compared, idle.snapshot().states().count());
    assert_eq!(survey.scanned, idle.tree().nodes().len());
    assert!(idle.snapshot().delta().changed().is_empty());
    let survey = idle.delta().survey();
    assert_eq!(survey.compared, 0);
    assert_eq!(survey.scanned, 0);
    assert_eq!(survey.indexed, 0);
    assert!(idle.delta().changed().is_empty());

    let first = test.document.find("first").unwrap();
    let text = test.document.entry(first).unwrap().children[0];
    test.document.replace(text, "A longer line").unwrap();
    let changed = test.check();
    assert_eq!(
        changed.snapshot().collection().visited,
        changed.snapshot().states().count()
    );
    assert_eq!(changed.snapshot().collection().reused, 0);
    let survey = changed.snapshot().impact().unwrap().survey();
    assert_eq!(survey.compared, changed.tree().nodes().len());
    assert_eq!(survey.topology, changed.tree().edges().len());
    assert_eq!(survey.indexed, changed.tree().edges().len());
    assert!(survey.routed > 0);
    assert!(survey.routed < survey.indexed);
    let survey = changed.snapshot().delta().survey();
    assert_eq!(survey.compared, changed.snapshot().states().count());
    assert_eq!(survey.scanned, idle.tree().nodes().len());
    assert!(!changed.snapshot().delta().changed().is_empty());
    let survey = changed.delta().survey();
    assert!(survey.compared >= branches);
    assert_eq!(survey.scanned, branches);
    assert!(survey.indexed > 0);
    assert!(!changed.delta().changed().is_empty());
}

#[test]
fn projection() {
    let mut test = Harness::new(
        "<div id='root'><p id='first'>A</p><div class='bound' id='a'>A</div><div class='bound' id='b'>B</div></div>",
        ".bound { width:32px; height:16px; contain:size layout paint; }",
    );
    let config = config::Store::default();
    config.set("layout.impact", true.into());
    config.set(
        "layout.fragments",
        serde_json::to_value(shared::cache::Policy::bounded(0, 0)).unwrap(),
    );
    test.pipeline = Pipeline::configured(Text, &test.manager, config).unwrap();
    let first = test.check();
    let root = test.document.find("root").unwrap();
    let firstnode = test.document.find("first").unwrap();
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let aid = first.tree().source(a).next().unwrap().id;
    let bid = first.tree().source(b).next().unwrap().id;
    assert!(first.delta().added().contains(&aid));
    assert!(first.delta().added().contains(&bid));

    let idle = test.check();
    assert!(idle.delta().changed().is_empty());
    test.document
        .attribute(a, "style", "background-color: #ff0000;")
        .unwrap();
    let styled = test.check();
    assert!(styled.delta().updated().contains(&aid));
    assert!(styled.snapshot().delta().changed().is_empty());

    test.document
        .replace(firstnode, "A line long enough to wrap across the viewport")
        .unwrap();
    let relocated = test.check();
    assert!(relocated.delta().relocated().contains(&aid));
    assert!(relocated.delta().relocated().contains(&bid));

    test.document.append(root, a).unwrap();
    let ordered = test.check();
    assert!(ordered.delta().reordered().contains(&aid));
    assert!(ordered.delta().reordered().contains(&bid));

    test.document.remove(root, b).unwrap();
    let removed = test.check();
    assert!(removed.delta().retired().contains(&bid));
    assert!(!removed.delta().changed().contains(&bid));
}

#[test]
fn scaling() {
    for scale in [64, 512, 4096] {
        scalecheck(scale);
    }
}

fn scalecheck(scale: usize) {
    let rows = (scale - 4) / 2;
    let mut document = scaledoc(rows);
    let manager = Manager::default();
    let sheet = Sheet::default();
    let mut capture = Capture::new(Resolver::managed(sheet, &manager).unwrap());
    let config = config::Store::default();
    config.set("layout.impact", true.into());
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();

    let first = scalebuild(&document, &mut capture, &mut pipeline);
    let nodes = first.tree().nodes().len();
    let edges = first.tree().edges().len();
    let idle = scalebuild(&document, &mut capture, &mut pipeline);
    assert_eq!(idle.snapshot().collection().visited, 0);
    assert_eq!(idle.snapshot().collection().reused, nodes);
    let impact = idle.snapshot().impact().unwrap().survey();
    assert_eq!(impact.compared, nodes);
    assert_eq!(impact.topology, edges);
    assert_eq!(impact.indexed, 0);
    assert_eq!(impact.routed, 0);
    assert_eq!(idle.snapshot().delta().survey().compared, nodes);
    assert_eq!(idle.snapshot().delta().survey().scanned, nodes);
    assert_eq!(idle.delta().survey().compared, 0);
    assert_eq!(idle.delta().survey().scanned, 0);
    assert_eq!(idle.delta().survey().indexed, 0);

    let row = document.find(&format!("row{}", rows / 2)).unwrap();
    let text = document.entry(row).unwrap().children[0];
    document
        .replace(
            text,
            "changed content long enough to wrap across the viewport",
        )
        .unwrap();
    let changed = scalebuild(&document, &mut capture, &mut pipeline);
    assert_eq!(changed.snapshot().collection().visited, nodes);
    assert_eq!(changed.snapshot().collection().reused, 0);
    let impact = changed.snapshot().impact().unwrap();
    assert!(!impact.seeds().is_empty());
    assert!(impact.survey().routed > 0);
    assert!(impact.missed().is_empty());
    assert!(!changed.snapshot().delta().changed().is_empty());
    assert!(!changed.delta().changed().is_empty());

    let root = document.find("root").unwrap();
    document.append(root, row).unwrap();
    let structured = scalebuild(&document, &mut capture, &mut pipeline);
    assert_eq!(structured.tree().nodes().len(), nodes);
    assert!(!structured.tree().delta().reordered().is_empty());
    assert!(!structured.delta().reordered().is_empty());
    let impact = structured.snapshot().impact().unwrap();
    assert!(impact.survey().indexed > 0);
    assert!(impact.missed().is_empty());
}

fn scaledoc(count: usize) -> Document {
    let mut document = Document::default();
    let root = document.element("div").unwrap();
    document.attribute(root, "id", "root").unwrap();
    document.append(document.root(), root).unwrap();
    for index in 0..count {
        let row = document.element("p").unwrap();
        document
            .attribute(row, "id", &format!("row{index}"))
            .unwrap();
        document.replace(row, &format!("row {index}")).unwrap();
        document.append(root, row).unwrap();
    }
    document
}

fn scalebuild(document: &Document, capture: &mut Capture, pipeline: &mut Pipeline<Text>) -> Layout {
    capture
        .update(document, |input| {
            pipeline.build(
                input,
                Constraints::viewport(Extent {
                    width: 128,
                    height: 128,
                }),
            )
        })
        .unwrap()
}
