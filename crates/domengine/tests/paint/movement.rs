use super::{Harness, extent, records};
use graphics::shared::{Backend, Cpu, Visual};
use std::sync::Arc;

fn contents(items: &[graphics::shared::Item], id: u64) -> Option<Arc<[graphics::shared::Item]>> {
    items.iter().find_map(|item| match item {
        graphics::shared::Item::Group { space, items, .. } if space.id == id => Some(items.clone()),
        graphics::shared::Item::Group { items, .. } => contents(items, id),
        _ => None,
    })
}

#[test]
fn sharing() {
    let mut test = Harness::new(
        "<div><p id='prefix' style='height: 16px;'>Prefix</p><div id='body' style='padding: 4px; opacity: 0.5; height: 48px; overflow: hidden;'><p id='text'>Alpha beta</p><p>Gamma delta</p></div></div>",
    );
    let body = test.document.find("body").unwrap().id() as u64 * 3 + 1;
    let prefix = test.document.find("prefix").unwrap();
    let original = test.check(Visual::new(extent()));
    let content = contents(original.scene().items(), body).unwrap();
    let pixels = Cpu.render(&original).unwrap();
    for height in [48, 0, 32, 16] {
        test.document
            .attribute(prefix, "style", &format!("height: {height}px;"))
            .unwrap();
        let moved = test.check(Visual::new(extent()));
        assert!(test.painter.inspect().relocated > 0);
        assert!(test.painter.inspect().placed < test.painter.inspect().relocated);
        assert!(Arc::ptr_eq(
            &content,
            &contents(moved.scene().items(), body).unwrap()
        ));
    }
    test.document
        .replace(test.document.find("text").unwrap(), "Different words")
        .unwrap();
    let changed = test.check(Visual::new(extent()));
    assert!(!Arc::ptr_eq(
        &content,
        &contents(changed.scene().items(), body).unwrap()
    ));
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn descendants() {
    let mut test = Harness::new(
        "<div><p id='prefix' style='height: 16px;'>Before</p><div id='body' style='height: 80px; overflow: hidden; opacity: 0.5; translate: 2px 4px;'><div><p id='a'>Alpha beta</p><p id='b'>Gamma delta</p><input id='editor' style='width: 40px;'></div></div></div>",
    );
    let prefix = test.document.find("prefix").unwrap();
    let a = test.document.find("a").unwrap();
    let editor = test.document.find("editor").unwrap();
    test.document.value(editor, "ABCDEFGHIJKL").unwrap();
    test.document.focus(Some(editor)).unwrap();
    test.document.caret(editor, 9).unwrap();
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    let before = records(original.scene().items());
    for (height, text) in [
        (48, "A"),
        (8, "Changed words"),
        (32, "Again"),
        (0, "Alpha beta"),
    ] {
        test.document
            .attribute(prefix, "style", &format!("height: {height}px;"))
            .unwrap();
        let moved = test.check(Visual::new(extent()));
        assert!(test.painter.inspect().relocated > 0);
        assert!(
            records(moved.scene().items())
                .iter()
                .any(|record| before.iter().any(|old| Arc::ptr_eq(old, record)))
        );
        test.document.replace(a, text).unwrap();
        test.check(Visual::new(extent()));
        test.document.caret(editor, height % 10).unwrap();
        test.check(Visual::new(extent()));
        test.check(Visual::new(extent()));
        assert_eq!(test.painter.inspect().relocated, 0);
    }
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn lifecycle() {
    let mut test = Harness::new(
        "<div id='root'><div id='a'><p>Alpha</p></div><div id='b'><p>Beta</p></div><div id='c'><p>Gamma</p></div></div>",
    );
    let root = test.document.find("root").unwrap();
    let a = test.document.find("a").unwrap();
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    for enabled in [true, false, true, true] {
        test.painter.enable(enabled);
        test.document.append(root, a).unwrap();
        test.check(Visual::new(extent()));
        if !enabled {
            assert_eq!(test.painter.inspect().reused, 0);
            assert_eq!(test.painter.inspect().relocated, 0);
        }
        test.document.remove(root, a).unwrap();
        test.check(Visual::new(extent()));
        test.document.append(root, a).unwrap();
        test.check(Visual::new(extent()));
    }
    test.painter.close();
    test.document
        .append(root, test.document.find("b").unwrap())
        .unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.painter.inspect().relocated, 0);
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn routing() {
    use domengine::Constraints;
    let mut test = Harness::new(&format!(
        "<div id='body' style='height: 64px; overflow: hidden; opacity: 0.5;'>{}</div>",
        "<p>Alpha beta</p>".repeat(128)
    ));
    let body = test.document.find("body").unwrap();
    let space = body.id() as u64 * 3 + 1;
    let original = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&original).unwrap();
    for (translation, opacity) in [
        ("4px 2px", "0.5"),
        ("4px 2px", "0"),
        ("-2px 5px", "0.25"),
        ("0px 0px", "1"),
    ] {
        let held = test
            .capture
            .update(&test.document, |input| {
                test.pipeline.build(input, Constraints::viewport(extent()))
            })
            .unwrap();
        let recorded = test.painter.build(&held).unwrap();
        let children = contents(&recorded, space).unwrap();
        test.document
            .attribute(
                body,
                "style",
                &format!(
                    "height: 64px; overflow: hidden; opacity: {opacity}; translate: {translation};"
                ),
            )
            .unwrap();
        let moved = test.check(Visual::new(extent()));
        assert_eq!(test.painter.inspect().routed, 1);
        assert!(Arc::ptr_eq(
            &children,
            &contents(moved.scene().items(), space).unwrap()
        ));
        drop(held);
    }
    for (index, extra) in ["background-color: #123456;", "height: 48px;"]
        .into_iter()
        .enumerate()
    {
        let held = test
            .capture
            .update(&test.document, |input| {
                test.pipeline.build(input, Constraints::viewport(extent()))
            })
            .unwrap();
        test.painter.build(&held).unwrap();
        test.document
            .attribute(
                body,
                "style",
                &format!(
                    "height: 64px; overflow: hidden; translate: {}px 8px; {extra}",
                    index + 8
                ),
            )
            .unwrap();
        test.check(Visual::new(extent()));
        assert_eq!(test.painter.inspect().routed, 0);
        drop(held);
    }
    let held = test
        .capture
        .update(&test.document, |input| {
            test.pipeline.build(input, Constraints::viewport(extent()))
        })
        .unwrap();
    test.painter.build(&held).unwrap();
    test.document.replace(body, "Changed content").unwrap();
    test.document
        .attribute(
            body,
            "style",
            "height: 48px; overflow: hidden; translate: 24px 8px;",
        )
        .unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.painter.inspect().routed, 0);
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), pixels);
}

#[test]
fn bypass() {
    let mut test = Harness::new("<div id='body'><p>Alpha</p></div>");
    let config = config::Store::default();
    config.set("paint.routing", false.into());
    test.painter = domengine::Painter::configured(&test.manager, config.clone()).unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.painter.inspect().checked, 0);
    test.document
        .attribute(
            test.document.find("body").unwrap(),
            "style",
            "translate: 4px 2px;",
        )
        .unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.painter.inspect().checked, 0);
    assert_eq!(test.painter.inspect().routed, 0);
    config.set("paint.routing", (-1).into());
    let layout = test
        .capture
        .update(&test.document, |input| {
            test.pipeline
                .build(input, domengine::Constraints::viewport(extent()))
        })
        .unwrap();
    assert!(test.painter.build(&layout).is_err());
}
