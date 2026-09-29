use super::Harness;
use domengine::{
    Selection,
    layout::{Cause, Mode},
};

#[test]
fn structure() {
    let mut test = Harness::new(
        "<div id='a' style='color: #ff0000;'><p id='child'>A</p></div><div id='b' style='color: #0000ff;'><p>B</p></div>",
        "",
    );
    test.check();
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let child = test.document.find("child").unwrap();
    test.document.append(b, child).unwrap();
    test.check();
    test.document.remove(b, child).unwrap();
    test.check();
    test.document.replace(child, "Detached").unwrap();
    test.check();
    assert_eq!(test.capture.inspect().rebuilt, 0);
    test.document.append(a, child).unwrap();
    test.check();
    let text = test.document.entry(child).unwrap().children[0];
    test.document.append(b, text).unwrap();
    let actual = test.check();
    let rect = actual
        .fragments()
        .iter()
        .find(|fragment| fragment.node == Some(text.id()))
        .unwrap()
        .rect;
    assert_eq!(actual.hit(rect.x + 1, rect.y + 1), Some(b));
    test.document
        .attribute(b, "style", "display: none;")
        .unwrap();
    test.document.replace(text, "Hidden").unwrap();
    test.check();
    test.document
        .attribute(b, "style", "display: block;")
        .unwrap();
    test.check();
}

#[test]
fn editing() {
    let mut test = Harness::new("<div><input id='a'><input id='b'><p>Static</p></div>", "");
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    test.document.value(a, "ABC").unwrap();
    test.check();
    test.document.focus(Some(a)).unwrap();
    test.document
        .select(
            a,
            Selection {
                anchor: 0,
                focus: 2,
            },
        )
        .unwrap();
    test.check();
    assert_eq!(test.capture.inspect().styled, 0);
    assert_eq!(test.pipeline.inspect().computed, 0);
    test.document.focus(Some(b)).unwrap();
    test.check();
    assert_eq!(test.capture.inspect().styled, 0);
    assert_eq!(test.pipeline.inspect().computed, 0);
    test.document.value(b, "Value").unwrap();
    test.check();
    assert!(test.pipeline.inspect().computed > 0);
    let parent = test.document.entry(b).unwrap().parent.unwrap();
    test.document.remove(parent, b).unwrap();
    test.check();
    assert_eq!(test.document.focused(), None);
}

#[test]
fn coalescing() {
    let mut test = Harness::new(
        "<div id='a'><p id='child'>ABC</p><p>DEF</p></div><div id='b'></div>",
        "",
    );
    test.check();
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let child = test.document.find("child").unwrap();
    test.document.append(b, child).unwrap();
    test.document
        .attribute(child, "style", "color: #ff0000;")
        .unwrap();
    test.document.append(a, child).unwrap();
    test.document
        .attribute(child, "style", "color: #0000ff;")
        .unwrap();
    test.document.replace(child, "Final").unwrap();
    test.check();
    let report = test.capture.inspect();
    assert!(report.changes > report.rebuilt);
    assert_eq!(report.styled, 2);
    assert_eq!(report.rebuilt, 5);
}

#[test]
fn sequences() {
    let mut test = Harness::new(
        "<div id='a'><p id='x'>X</p><p id='y'>Y</p></div><div id='b'><input id='editor'></div>",
        ".red { color: #ff0000; } .wide { width: 96px; }",
    );
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let x = test.document.find("x").unwrap();
    let y = test.document.find("y").unwrap();
    let editor = test.document.find("editor").unwrap();
    test.check();
    for index in 0usize..96 {
        match index % 8 {
            0 => test.document.replace(x, &format!("Value{index}")).unwrap(),
            1 => test.document.append(b, x).unwrap(),
            2 => test
                .document
                .attribute(b, "style", "color: #0000ff;")
                .unwrap(),
            3 => test.document.attribute(x, "class", "red").unwrap(),
            4 => test.document.append(a, x).unwrap(),
            5 => test
                .document
                .attribute(
                    y,
                    "class",
                    if index.is_multiple_of(3) { "wide" } else { "" },
                )
                .unwrap(),
            6 => test.document.value(editor, &format!("{index}")).unwrap(),
            _ => test.document.focus(Some(editor)).unwrap(),
        }
        if index.is_multiple_of(3) {
            test.check();
        }
    }
    test.check();
}

#[test]
fn planning() {
    let mut test = Harness::new("<div id='root'><p id='a'>A</p><p id='b'>B</p></div>", "");
    let first = test.check();
    assert_eq!(first.tree().plan().mode(), Mode::Full);
    assert_eq!(
        first.tree().plan().frontier().len(),
        first.tree().nodes().len()
    );
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let aid = first.tree().source(a).next().unwrap().id;
    let text = test.document.entry(a).unwrap().children[0];
    test.document.replace(text, "Changed").unwrap();
    let next = test.check();
    let plan = next.tree().plan();
    assert_eq!(plan.mode(), Mode::Delta);
    assert_eq!(plan.causes(), [Cause::Content]);
    assert_eq!(plan.seeds(), [next.tree().source(text).next().unwrap().id]);
    assert!(
        next.tree()
            .source(a)
            .all(|node| plan.frontier().contains(&node.id))
    );
    assert!(
        next.tree()
            .source(b)
            .all(|node| !plan.frontier().contains(&node.id))
    );
    let idle = test.check();
    assert_eq!(idle.tree().plan().mode(), Mode::Delta);
    assert!(idle.tree().plan().frontier().is_empty());
    let config = config::Store::default();
    config.set("dom.history", 0.into());
    test.document.configure(&config).unwrap();
    test.document.replace(text, "Reset").unwrap();
    let reset = test.check();
    assert_eq!(reset.tree().plan().mode(), Mode::Full);
    assert_eq!(reset.tree().source(a).next().unwrap().id, aid);
    assert_eq!(
        reset.tree().plan().frontier().len(),
        reset.tree().nodes().len()
    );
}

#[test]
fn inlineplan() {
    let mut test = Harness::new(
        "<div id='line'><span>A</span><span id='tail'>B</span></div>",
        "",
    );
    test.check();
    let line = test.document.find("line").unwrap();
    let tail = test.document.find("tail").unwrap();
    let text = test.document.entry(tail).unwrap().children[0];
    test.document.replace(text, "Changed").unwrap();
    let layout = test.check();
    let plan = layout.tree().plan();
    let anon = layout
        .tree()
        .source(line)
        .find(|node| node.role == domengine::layout::Role::Anonymous)
        .unwrap();
    assert!(plan.frontier().contains(&anon.id));
}

#[test]
fn diffing() {
    let mut test = Harness::new("<div id='root'><p id='a'>A</p><p id='b'>B</p></div>", "");
    let first = test.check();
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let aid = first.tree().source(a).next().unwrap().id;
    let bid = first.tree().source(b).next().unwrap().id;
    assert_eq!(
        first.snapshot().delta().changed().len(),
        first.snapshot().states().count()
    );
    let idle = test.check();
    assert!(idle.snapshot().delta().changed().is_empty());
    assert!(idle.snapshot().impact().unwrap().seeds().is_empty());
    test.document
        .replace(a, "A much longer line of content")
        .unwrap();
    let changed = test.check();
    assert!(changed.snapshot().delta().changed().contains(&aid));
    assert!(changed.snapshot().delta().changed().contains(&bid));
    assert!(
        changed
            .snapshot()
            .impact()
            .unwrap()
            .affected()
            .contains(&bid)
    );
    assert!(changed.snapshot().impact().unwrap().missed().is_empty());
    assert!(changed.snapshot().delta().moved().contains(&bid));
    assert!(!changed.snapshot().delta().resized().contains(&bid));
    let before = idle.snapshot().metric(bid).unwrap();
    let after = changed.snapshot().metric(bid).unwrap();
    assert_eq!(before.size(), after.size());
    assert_ne!(before.origin(), after.origin());
    let root = test.document.find("root").unwrap();
    test.document.remove(root, b).unwrap();
    let removed = test.check();
    assert!(removed.snapshot().delta().retired().contains(&bid));
}

#[test]
fn treedelta() {
    let mut test = Harness::new(
        "<div id='root'><p id='a'>A</p><p id='b'>B</p></div><div id='target'></div>",
        "",
    );
    let first = test.check();
    let root = test.document.find("root").unwrap();
    let target = test.document.find("target").unwrap();
    let a = test.document.find("a").unwrap();
    let b = test.document.find("b").unwrap();
    let aid = first.tree().source(a).next().unwrap().id;
    let bid = first.tree().source(b).next().unwrap().id;
    assert_eq!(
        first.tree().delta().added().len(),
        first.tree().nodes().len()
    );

    let idle = test.check();
    assert!(idle.tree().delta().changed().is_empty());
    test.document
        .attribute(a, "style", "color: #ff0000;")
        .unwrap();
    let styled = test.check();
    assert!(styled.tree().delta().updated().contains(&aid));
    assert!(styled.snapshot().delta().changed().is_empty());
    assert!(styled.snapshot().impact().unwrap().seeds().is_empty());

    test.document.append(root, a).unwrap();
    let ordered = test.check();
    assert!(ordered.tree().delta().reordered().contains(&aid));
    assert!(ordered.tree().delta().reordered().contains(&bid));
    assert!(ordered.tree().delta().moved().is_empty());

    test.document.remove(root, b).unwrap();
    let removed = test.check();
    assert!(removed.tree().delta().retired().contains(&bid));
    assert!(!removed.tree().delta().changed().contains(&bid));

    test.document.append(target, a).unwrap();
    let moved = test.check();
    assert_eq!(moved.tree().source(a).next().unwrap().id, aid);
    assert!(moved.tree().delta().moved().contains(&aid));
    assert!(!moved.tree().delta().added().contains(&aid));
    assert!(!moved.tree().delta().retired().contains(&aid));
}
