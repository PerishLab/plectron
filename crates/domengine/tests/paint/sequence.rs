use super::{Harness, extent, records};
use domengine::{Constraints, Content, Input, Layout, Sheet};
use graphics::shared::{Backend, Cpu, Frame, Ink, Scene, Visual};

#[test]
fn placement() {
    let mut test = Harness::new("<p id='text' style='margin: 12px; padding: 4px;'>AB</p>");
    let input = Input::capture(&test.document, &Sheet::default()).unwrap();
    let layout = Layout::build(&input, Constraints::viewport(extent()), &fixtures::Text).unwrap();
    let (origin, glyphs) = layout
        .fragments()
        .iter()
        .find_map(|fragment| match &fragment.content {
            Content::Text { glyphs, .. } => Some((fragment.origin(), glyphs)),
            _ => None,
        })
        .unwrap();
    assert_eq!(origin, [16, 16]);
    assert_eq!((glyphs[0].x, glyphs[0].y), (0.into(), 0.into()));
    let items = test.painter.build(&layout).unwrap();
    let record = records(&items)
        .into_iter()
        .find(|record| matches!(record.ink(), Ink::Text { .. }))
        .unwrap();
    let Ink::Text {
        glyphs: recorded, ..
    } = record.ink()
    else {
        unreachable!()
    };
    assert!(std::sync::Arc::ptr_eq(glyphs, recorded));
    let frame = Frame::new(
        Scene::new(layout.extent(), layout.revision(), items).unwrap(),
        Visual::new(extent()),
    )
    .unwrap();
    let old = Cpu.render(&frame).unwrap();
    let node = test.document.find("text").unwrap();
    test.document
        .attribute(node, "style", "margin: 20px; padding: 4px;")
        .unwrap();
    let moved = test.check(Visual::new(extent()));
    assert!(
        records(moved.scene().items())
            .iter()
            .any(|candidate| std::sync::Arc::ptr_eq(candidate, &record))
    );
    assert_ne!(Cpu.render(&moved).unwrap(), old);
    test.painter.close();
    drop(layout);
    assert_eq!(Cpu.render(&frame).unwrap(), old);
}

#[test]
fn order() {
    let mut test = Harness::new(
        "<div style='background-color: #ff0000;'><p>A<strong>B</strong><span style='color: #008000;'>CD</span><img id='atom' src='inline.png' style='display: inline; padding: 2px;'>EF</p><input id='editor'></div><p style='background-color: #0000ff;'>GH</p>",
    );
    let atom = test.document.find("atom").unwrap();
    test.document
        .image(
            atom,
            "inline.png",
            graphics::shared::Raster::rgba(
                graphics::shared::Extent {
                    width: 4,
                    height: 6,
                },
                &[0, 255, 0, 255].repeat(24),
            )
            .unwrap(),
        )
        .unwrap();
    let editor = test.document.find("editor").unwrap();
    test.document.value(editor, "ABCDE").unwrap();
    test.document.focus(Some(editor)).unwrap();
    test.document
        .select(
            editor,
            domengine::Selection {
                anchor: 1,
                focus: 4,
            },
        )
        .unwrap();
    let input = Input::capture(&test.document, &Sheet::default()).unwrap();
    for width in [48, 96] {
        let layout = Layout::build(
            &input,
            Constraints::viewport(graphics::shared::Extent { width, ..extent() }),
            &fixtures::Text,
        )
        .unwrap();
        let fragments = layout.fragments();
        let mut order = Vec::new();
        visit(fragments, None, &mut order);
        assert_eq!(order, (0..fragments.len()).collect::<Vec<_>>());
        assert!(
            fragments
                .iter()
                .any(|f| matches!(f.content, Content::Selection { .. }))
        );
        assert!(
            fragments
                .iter()
                .any(|f| matches!(f.content, Content::Caret { .. }))
        );
        let expected: Vec<_> = fragments
            .iter()
            .flat_map(|fragment| match fragment.content {
                Content::Box { .. } => vec![fragment.space, fragment.space + 1],
                Content::Inline { .. } => vec![fragment.space],
                _ => Vec::new(),
            })
            .collect();
        let scene = Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap();
        let frame = Frame::new(scene, Visual::new(layout.extent())).unwrap();
        assert_eq!(
            frame
                .mappings()
                .iter()
                .skip(1)
                .map(|mapping| mapping.id)
                .collect::<Vec<_>>(),
            expected
        );
    }
}
fn visit(fragments: &[domengine::Fragment], parent: Option<usize>, order: &mut Vec<usize>) {
    for fragment in fragments
        .iter()
        .filter(|fragment| fragment.parent == parent)
    {
        order.push(fragment.id);
        visit(fragments, Some(fragment.id), order);
    }
}

#[test]
fn buffers() {
    let mut test = Harness::new(
        "<p>ABCDEFGHIJKLMNOPQRSTUVWXYZ</p><p>AB</p><p id='change'>ABC</p><p>ABCDEFGHIJKLMNOPQRSTUVWXYZ</p>",
    );
    let original = test.check(Visual::new(extent()));
    let expected = Cpu.render(&original).unwrap();
    let target = test.document.find("change").unwrap();
    for text in ["AB", "ABCDEF", "A", "ABCDEFGHIJKLMNOPQRSTUVWXYZ", "ABC"] {
        test.document.replace(target, text).unwrap();
        let frame = test.check(Visual::new(extent()));
        for record in records(frame.scene().items()) {
            let Ink::Text { glyphs, .. } = record.ink() else {
                continue;
            };
            let Ink::Text { glyphs: cloned, .. } = record.ink().clone() else {
                unreachable!()
            };
            assert!(std::sync::Arc::ptr_eq(glyphs, &cloned));
        }
        assert_eq!(Cpu.render(&original).unwrap(), expected);
    }
    drop(test);
    assert_eq!(Cpu.render(&original).unwrap(), expected);
}
