use super::{Capture, Harness, Input, Pipeline, Sheet, Text, constraints, html, pixels};
use domengine::layout::{Edge, Relation};
use shared::cache::{Command, Manager};

#[test]
fn inference() {
    let document = html::parse("<div><p id='row'>same</p></div>").unwrap();
    let explicit =
        Sheet::parse("p { width:64px; height:32px; contain:size layout paint; }").unwrap();
    let inferred = Sheet::parse("p { width:64px; height:32px; overflow:hidden; }").unwrap();
    let row = document.find("row").unwrap();
    let mut declared = Pipeline::new(Text);
    let mut proven = Pipeline::new(Text);
    for demand in [Some([].into()), Some([row].into())] {
        declared.demand(demand.clone());
        proven.demand(demand);
        let expected = declared
            .build(
                &Input::capture(&document, &explicit).unwrap(),
                constraints(),
            )
            .unwrap();
        let actual = proven
            .build(
                &Input::capture(&document, &inferred).unwrap(),
                constraints(),
            )
            .unwrap();
        assert_eq!(actual.fragments(), expected.fragments());
        assert_eq!(pixels(&actual), pixels(&expected));
        assert_eq!(actual.boundaries().len(), 1);
    }
}

#[test]
fn dependencies() {
    let document = html::parse("<div><div class='boundary' id='a'><p id='x'>same</p></div><div class='boundary' id='b'><p id='y'>same</p></div></div>").unwrap();
    let sheet = Sheet::parse(".boundary { height:32px; contain:size layout paint; } p { height:16px; contain:size layout paint; }").unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let [a, b, x, y] = ["a", "b", "x", "y"].map(|id| document.find(id).unwrap());
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("layout.retention", false.into());
    config.set("layout.impact", true.into());
    config.set("layout.recurrence", 0.into());
    let mut engine = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let mut reference = Pipeline::new(Text);
    reference.enable(false);
    let mut rejected = 0;
    for demand in [
        None,
        Some(vec![]),
        Some(vec![a]),
        Some(vec![b, y]),
        Some(vec![a, x]),
        None,
    ] {
        let demand = demand.map(|nodes| nodes.into_iter().collect());
        engine.demand(demand.clone());
        reference.demand(demand);
        let expected = reference.build(&input, constraints()).unwrap();
        for _ in 0..2 {
            let actual = engine.build(&input, constraints()).unwrap();
            assert_eq!(actual.fragments(), expected.fragments());
            assert_eq!(pixels(&actual), pixels(&expected));
            assert!(actual.snapshot().impact().unwrap().missed().is_empty());
            rejected += engine.inspect().dependencies;
        }
        assert_eq!(engine.inspect().computed, 0);
        assert_eq!(engine.inspect().hits, 1);
    }
    assert!(rejected > 0);
    for cache in manager.inspect() {
        manager.command(cache.id, Command::Clear).unwrap();
    }
    engine.demand(Some([a].into()));
    reference.demand(Some([a].into()));
    assert_eq!(
        engine.build(&input, constraints()).unwrap().fragments(),
        reference.build(&input, constraints()).unwrap().fragments()
    );
    config.set("layout.dependencies", false.into());
    engine.demand(Some([b, y].into()));
    reference.demand(Some([b, y].into()));
    assert_eq!(
        engine.build(&input, constraints()).unwrap().fragments(),
        reference.build(&input, constraints()).unwrap().fragments()
    );
}
#[test]
fn identity() {
    let mut document = html::parse(
        "<div id='root'><div class='boundary' id='a'>same</div><div class='boundary' id='b'>same</div></div>",
    )
    .unwrap();
    let rules = ".boundary { height:32px; contain:size layout paint; }";
    let sheet = Sheet::parse(rules).unwrap();
    let mut capture = Capture::new(domengine::style::Resolver::new(
        Sheet::parse(rules).unwrap(),
    ));
    let mut engine = Pipeline::new(Text);
    let mut reference = Pipeline::new(Text);
    reference.enable(false);
    let root = document.find("root").unwrap();
    let a = document.find("a").unwrap();
    engine.demand(Some([a].into()));
    reference.demand(Some([a].into()));
    capture
        .update(&document, |input| engine.build(input, constraints()))
        .unwrap();
    document.append(root, a).unwrap();
    let actual = capture
        .update(&document, |input| engine.build(input, constraints()))
        .unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let expected = reference.build(&input, constraints()).unwrap();
    assert_eq!(actual.fragments(), expected.fragments());
    assert_eq!(pixels(&actual), pixels(&expected));
    assert_eq!(engine.inspect().retained, 0);
}

#[test]
fn locality() {
    let groups = (0..48)
        .map(|index| format!("<div><p id='row{index}'>same</p></div>"))
        .collect::<String>();
    let document = html::parse(&format!("<div>{groups}</div>")).unwrap();
    let sheet = Sheet::parse("p { height:32px; contain:size layout paint; }").unwrap();
    let input = Input::capture(&document, &sheet).unwrap();
    let mut engine = Pipeline::new(Text);
    let mut reference = Pipeline::new(Text);
    reference.enable(false);
    for index in [0, 1, 47, 24, 0] {
        let node = document.find(&format!("row{index}")).unwrap();
        engine.demand(Some([node].into()));
        reference.demand(Some([node].into()));
        let actual = engine.build(&input, constraints()).unwrap();
        let expected = reference.build(&input, constraints()).unwrap();
        assert_eq!(actual.fragments(), expected.fragments());
        assert_eq!(pixels(&actual), pixels(&expected));
        assert!(engine.inspect().hits >= 46);
        assert_eq!(engine.inspect().charged, reference.inspect().charged);
    }
}

#[test]
fn boundary() {
    let mut test = Harness::new(
        "<div id='root'><div id='limit'><p id='text'>A</p></div></div>",
        "#limit { width:64px; height:32px; contain:size layout paint; overflow:hidden; }",
    );
    let first = test.check();
    let root = test.document.find("root").unwrap();
    let limit = test.document.find("limit").unwrap();
    let text = test.document.find("text").unwrap();
    let rid = first.tree().source(root).next().unwrap().id;
    let lid = first.tree().source(limit).next().unwrap().id;
    test.document
        .replace(text, "A much longer line that changes isolated content")
        .unwrap();
    let next = test.check();
    assert!(next.tree().plan().frontier().contains(&rid));
    assert!(!next.snapshot().impact().unwrap().affected().contains(&rid));
    assert!(next.snapshot().impact().unwrap().missed().is_empty());
    assert!(!next.snapshot().delta().changed().contains(&rid));
    assert!(!next.snapshot().delta().resized().contains(&lid));
}

#[test]
fn graph() {
    let mut test = Harness::new(
        "<div id='root'><p id='first'>A</p><p id='limit'>B</p></div>",
        "#limit { width:32px; height:16px; contain:size layout paint; }",
    );
    let layout = test.check();
    let tree = layout.tree();
    let [root, first, limit] = ["root", "first", "limit"].map(|name| {
        tree.source(test.document.find(name).unwrap())
            .next()
            .unwrap()
            .id
    });
    assert!(tree.edges().contains(&Edge {
        from: first,
        to: root,
        relation: Relation::Intrinsic,
    }));
    assert!(tree.edges().contains(&Edge {
        from: root,
        to: limit,
        relation: Relation::Constraint,
    }));
    assert!(tree.edges().contains(&Edge {
        from: first,
        to: limit,
        relation: Relation::Flow,
    }));
    assert!(
        tree.source(test.document.find("limit").unwrap())
            .next()
            .unwrap()
            .boundary
    );
    let edges = tree.edges().to_vec();
    let versions = [root, first, limit].map(|id| {
        tree.nodes()
            .iter()
            .find(|entity| entity.id == id)
            .unwrap()
            .version
    });
    let text = test
        .document
        .entry(test.document.find("limit").unwrap())
        .unwrap()
        .children[0];
    let textid = tree.source(text).next().unwrap().id;
    let textver = tree.source(text).next().unwrap().version;
    test.document.replace(text, "Changed").unwrap();
    let changed = test.check();
    assert_eq!(changed.tree().edges(), edges);
    assert_eq!(
        changed
            .tree()
            .source(test.document.find("root").unwrap())
            .next()
            .unwrap()
            .version,
        versions[0]
    );
    assert_eq!(
        changed
            .tree()
            .source(test.document.find("first").unwrap())
            .next()
            .unwrap()
            .version,
        versions[1]
    );
    assert_eq!(
        changed
            .tree()
            .source(test.document.find("limit").unwrap())
            .next()
            .unwrap()
            .version,
        versions[2]
    );
    assert_ne!(
        changed
            .tree()
            .nodes()
            .iter()
            .find(|entity| entity.id == textid)
            .unwrap()
            .version,
        textver
    );
    let version = changed
        .tree()
        .source(test.document.find("first").unwrap())
        .next()
        .unwrap()
        .version;
    test.document
        .attribute(
            test.document.find("first").unwrap(),
            "style",
            "opacity:0.5;",
        )
        .unwrap();
    assert_eq!(
        test.check()
            .tree()
            .source(test.document.find("first").unwrap())
            .next()
            .unwrap()
            .version,
        version
    );
    let config = config::Store::default();
    config.set("dom.history", 0.into());
    test.document.configure(&config).unwrap();
    test.document
        .attribute(
            test.document.find("first").unwrap(),
            "style",
            "opacity:0.25;",
        )
        .unwrap();
    assert_eq!(
        test.check()
            .tree()
            .source(test.document.find("first").unwrap())
            .next()
            .unwrap()
            .version,
        version
    );
}
