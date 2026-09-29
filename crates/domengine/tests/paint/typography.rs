use super::{Harness, extent};
use graphics::shared::{Backend, Cpu, Visual};

#[test]
fn revisions() {
    let mut test = Harness::new(
        "<div id='root'><p id='text'>AB CD</p><p id='shield' style='font-size: 8px; line-height: 12px;'>AB</p><input id='input'></div>",
    );
    let root = test.document.find("root").unwrap();
    let text = test.document.find("text").unwrap();
    let input = test.document.find("input").unwrap();
    test.document.value(input, "AB").unwrap();
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    for style in [
        "font-size: 24px; line-height: 1.5;",
        "font-size: 24px; line-height: 1.5; font-weight: bold;",
        "font-size: 24px; line-height: 1.5; font-weight: bold; letter-spacing: 3px;",
        "font-size: 24px; line-height: 12px; text-align: end;",
        "font-size: 24px; line-height: 8px; translate: 8px 16px;",
        "font-size: 17px; line-height: 2; white-space: pre-wrap; overflow-wrap: anywhere;",
    ] {
        test.document.attribute(root, "style", style).unwrap();
        test.check(Visual::new(extent()));
        assert!(test.pipeline.inspect().computed > 0);
        test.check(Visual::new(extent()));
        assert_eq!(test.pipeline.inspect().computed, 0);
        assert_eq!(test.painter.inspect().recorded, 0);
    }
    test.document
        .attribute(root, "style", "white-space: normal;")
        .unwrap();
    let child = test.document.entry(text).unwrap().children[0];
    for source in ["  \n  ", "restored", "", "A\n B"] {
        test.document.replace(child, source).unwrap();
        test.check(Visual::new(extent()));
        test.document
            .attribute(text, "style", "white-space: pre;")
            .unwrap();
        test.check(Visual::new(extent()));
        test.document
            .attribute(text, "style", "white-space: normal;")
            .unwrap();
        test.check(Visual::new(extent()));
    }
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn clusters() {
    let mut test = Harness::new(
        "<p id='text' style='font-family: lato; font-size: 28px; overflow-wrap: anywhere;'>Å ffi</p>",
    );
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.pipeline.inspect().computed, 0);
    assert_eq!(test.painter.inspect().recorded, 0);
    let node = test.document.find("text").unwrap();
    for text in ["é fi", "Å ffi", "ffi Å"] {
        test.document.replace(node, text).unwrap();
        test.check(Visual::new(extent()));
    }
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn binding() {
    let mut test = Harness::new("<p id='text' style='width: 120px; text-align: center;'>ABCDE</p>");
    let node = test.document.find("text").unwrap();
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    let record = super::text(&original);
    for width in [112, 104, 96] {
        test.document
            .attribute(
                node,
                "style",
                &format!("width: {width}px; text-align: center; color: #ff0000; overflow: hidden;"),
            )
            .unwrap();
        let frame = test.check(Visual::new(extent()));
        assert_eq!(test.pipeline.inspect().construction.text.retained, 1);
        assert_eq!(test.pipeline.inspect().construction.text.generated, 0);
        let current = super::text(&frame);
        let graphics::shared::Ink::Text { glyphs: before, .. } = record.ink() else {
            panic!()
        };
        let graphics::shared::Ink::Text { glyphs: after, .. } = current.ink() else {
            panic!()
        };
        assert!(std::sync::Arc::ptr_eq(before, after));
    }
    for style in [
        "width: 16px; overflow-wrap: anywhere;",
        "width: 112px; font-size: 24px;",
    ] {
        test.document.attribute(node, "style", style).unwrap();
        test.check(Visual::new(extent()));
        assert_eq!(test.pipeline.inspect().construction.text.retained, 0);
    }
    test.document.replace(node, "Different").unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.pipeline.inspect().construction.text.retained, 0);
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}
