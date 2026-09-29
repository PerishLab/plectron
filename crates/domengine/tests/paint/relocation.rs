use super::{Harness, extent};
use graphics::shared::{Backend, Cpu, Visual};

#[test]
fn ordering() {
    let mut test = Harness::new(
        "<div id='root' style='height: 100px; overflow: auto;'><div id='a'><p>Alpha beta</p><p>Second</p></div><div id='b'><p>Gamma</p></div><div id='c'><p>Delta</p><p>Extra</p></div></div>",
    );
    let root = test.document.find("root").unwrap();
    let nodes = ["a", "b", "c"].map(|id| test.document.find(id).unwrap());
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    for index in [0, 2, 1, 0, 1, 2] {
        test.document.append(root, nodes[index]).unwrap();
        test.check(Visual::new(extent()));
        assert!(test.pipeline.inspect().relocated > 0);
        assert!(test.painter.inspect().relocated > 0);
        assert!(test.painter.inspect().requests < 10);
        test.check(Visual::new(extent()));
        assert_eq!(test.pipeline.inspect().relocated, 0);
        assert_eq!(test.painter.inspect().relocated, 0);
    }
    test.document.remove(root, nodes[0]).unwrap();
    test.check(Visual::new(extent()));
    assert!(test.pipeline.inspect().relocated > 0);
    test.document.append(root, nodes[0]).unwrap();
    test.check(Visual::new(extent()));
    test.document.replace(nodes[1], "Changed content").unwrap();
    test.check(Visual::new(extent()));
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn clipping() {
    let mut test = Harness::new(
        "<div id='outer' style='height: 90px; overflow: hidden;'><p id='before' style='height: 16px;'>Prefix</p><div><p>Plain</p><div style='height: 20px; overflow: hidden;'><p>Clipped</p><p>Overflow</p></div><p>After</p></div></div>",
    );
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    let before = test.document.find("before").unwrap();
    let outer = test.document.find("outer").unwrap();
    for height in [48, 0, 32, 16] {
        test.document
            .attribute(before, "style", &format!("height: {height}px;"))
            .unwrap();
        test.document
            .attribute(
                outer,
                "style",
                &format!("height: {}px; overflow: hidden;", 64 + height),
            )
            .unwrap();
        test.check(Visual::new(extent()));
        assert!(test.pipeline.inspect().relocated > 0);
    }
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn invalidation() {
    let mut test = Harness::new(
        "<div id='root'><p id='prefix' style='height: 16px;'>Before</p><div id='body'><p>Long wrapping paragraph with text</p><p><strong>Inline</strong> text</p></div></div>",
    );
    test.check(Visual::new(extent()));
    let prefix = test.document.find("prefix").unwrap();
    let body = test.document.find("body").unwrap();
    for style in ["width: 48px;", "color: #ff0000;", "width: 96px;", ""] {
        test.document.attribute(body, "style", style).unwrap();
        test.check(Visual::new(extent()));
        test.document
            .attribute(prefix, "style", "height: 32px;")
            .unwrap();
        test.check(Visual::new(extent()));
        test.document
            .attribute(prefix, "style", "height: 16px;")
            .unwrap();
        test.check(Visual::new(extent()));
        assert!(test.pipeline.inspect().relocated > 0);
    }
    test.pipeline.enable(false);
    test.document
        .attribute(prefix, "style", "height: 0px;")
        .unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.pipeline.inspect().relocated, 0);
}

#[test]
fn editing() {
    let mut test = Harness::new(
        "<div><p id='prefix' style='height: 16px;'>Before</p><div id='body' style='width: 60px; height: 48px; overflow: auto;'><input id='editor' style='width: 40px;'><p>After</p></div></div>",
    );
    let prefix = test.document.find("prefix").unwrap();
    let editor = test.document.find("editor").unwrap();
    test.document.value(editor, "ABCDEFGHIJKLM").unwrap();
    test.document.focus(Some(editor)).unwrap();
    test.document
        .select(
            editor,
            domengine::Selection {
                anchor: 1,
                focus: 11,
            },
        )
        .unwrap();
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    for height in [64, 8, 32, 0] {
        test.document
            .attribute(prefix, "style", &format!("height: {height}px;"))
            .unwrap();
        test.check(Visual::new(extent()));
        assert!(test.pipeline.inspect().relocated > 0);
    }
    test.document.caret(editor, 2).unwrap();
    test.check(Visual::new(extent()));
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}
