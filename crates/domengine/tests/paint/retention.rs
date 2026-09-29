use super::{Harness, extent};
use graphics::shared::{Backend, Cpu, Visual};

#[test]
fn wrapping() {
    let mut test = Harness::new(
        "<div><p>AB</p><p id='wrapped' style='width: 16px;'>CD EF GH</p><p id='change'>IJ</p></div>",
    );
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    let node = test.document.find("change").unwrap();
    let wrapped = test.document.find("wrapped").unwrap();
    for text in ["IK", "IL", "IM"] {
        test.document.replace(node, text).unwrap();
        test.check(Visual::new(extent()));
        assert!(test.painter.inspect().reused > 0);
    }
    for color in ["#112233", "#445566", "#778899"] {
        test.document
            .attribute(
                wrapped,
                "style",
                &format!("width: 16px; background-color: {color};"),
            )
            .unwrap();
        test.check(Visual::new(extent()));
    }
    test.painter.close();
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn branches() {
    let source = format!(
        "<div id='root'>{}</div>",
        "<div><p>ABC</p><p>DEF</p></div>".repeat(24)
    );
    let mut test = Harness::new(&source);
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    test.check(Visual::new(extent()));
    assert!(test.painter.inspect().requests < 10);
    assert!(test.painter.inspect().reused > 100);
    let root = test.document.find("root").unwrap();
    let section = test.document.entry(root).unwrap().children[0];
    let paragraph = test.document.entry(section).unwrap().children[0];
    let text = test.document.entry(paragraph).unwrap().children[0];
    for value in ["ABD", "ABE", "ABF"] {
        test.document.replace(text, value).unwrap();
        test.check(Visual::new(extent()));
        assert!(test.pipeline.inspect().reused > 100);
        assert!(test.painter.inspect().requests < 10);
        assert!(test.painter.inspect().reused > 100);
    }
    test.document
        .attribute(section, "style", "height: 90px;")
        .unwrap();
    test.check(Visual::new(extent()));
    test.document
        .attribute(root, "style", "color: #112233;")
        .unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.painter.inspect().reused, 0);
    test.painter.close();
    test.pipeline.close();
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}
#[test]
fn provenance() {
    let mut test = Harness::new(
        "<div id='root'><p id='first'>ABC</p><p id='second'>DEF</p><input id='editor'></div>",
    );
    let first = test.document.find("first").unwrap();
    let second = test.document.find("second").unwrap();
    let editor = test.document.find("editor").unwrap();
    test.document.value(editor, "ABC").unwrap();
    test.check(Visual::new(extent()));
    test.check(Visual::new(extent()));
    test.document.replace(first, "ABC").unwrap();
    test.check(Visual::new(extent()));
    test.document
        .attribute(second, "style", "display: none;")
        .unwrap();
    test.check(Visual::new(extent()));
    test.document
        .attribute(second, "style", "display: block;")
        .unwrap();
    test.document.focus(Some(editor)).unwrap();
    test.check(Visual::new(extent()));
    test.document.caret(editor, 2).unwrap();
    test.check(Visual::new(extent()));
    test.document
        .attribute(
            test.document.find("root").unwrap(),
            "style",
            "height: 30px; overflow: hidden;",
        )
        .unwrap();
    test.check(Visual::new(extent()));
}
