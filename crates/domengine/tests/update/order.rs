use super::Harness;

#[test]
fn matching() {
    let mut test = Harness::new(
        "<div id='root'><p id='a'>Alpha</p><input id='b'><p id='c'>Gamma</p><p id='d'>Delta</p></div>",
        "p { height: 16px; } input { height: 16px; }",
    );
    let root = test.document.find("root").unwrap();
    let nodes = ["a", "b", "c", "d"].map(|id| test.document.find(id).unwrap());
    test.document.value(nodes[1], "Beta").unwrap();
    test.document.focus(Some(nodes[1])).unwrap();
    test.check();
    for (append, index) in [
        (true, 2),
        (true, 0),
        (false, 3),
        (false, 0),
        (true, 0),
        (true, 3),
        (true, 1),
        (false, 1),
        (false, 0),
        (false, 2),
        (false, 3),
        (true, 3),
        (true, 2),
        (true, 1),
        (true, 0),
    ] {
        if append {
            test.document.append(root, nodes[index]).unwrap();
        } else {
            test.document.remove(root, nodes[index]).unwrap();
        }
        test.check();
    }
}

#[test]
fn appearance() {
    let mut test = Harness::new("<div id='root'><p id='a'>A</p><p id='b'>B</p></div>", "");
    test.check();
    let root = test.document.find("root").unwrap();
    for style in [
        "color: #ff0000;",
        "background-color: #00ff00;",
        "width: 64px;",
        "display: none;",
        "display: flex;",
        "color: #0000ff;",
    ] {
        test.document.attribute(root, "style", style).unwrap();
        test.check();
    }
}

#[test]
fn convergence() {
    let source = format!("<div id='root'>{}</div>", "<p>A</p>".repeat(48));
    let mut test = Harness::new(&source, "p { height: 16px; }");
    test.check();
    let root = test.document.find("root").unwrap();
    let children = test.document.entry(root).unwrap().children.clone();
    for child in &children {
        let text = test.document.entry(*child).unwrap().children[0];
        test.document.replace(text, "B").unwrap();
    }
    test.document
        .attribute(root, "style", "color: #ff0000;")
        .unwrap();
    test.check();
    let report = test.capture.inspect();
    assert_eq!(report.ancestry, report.affected);
    assert_eq!(report.styled, report.nodes - 1);
    test.document.remove(test.document.root(), root).unwrap();
    for child in &children {
        let text = test.document.entry(*child).unwrap().children[0];
        test.document.replace(text, "C").unwrap();
    }
    test.check();
    assert_eq!(test.capture.inspect().affected, 1);
    test.document.append(test.document.root(), root).unwrap();
    test.check();
    assert_eq!(test.capture.inspect().affected, 2);
}
