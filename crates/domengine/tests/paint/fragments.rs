use super::{Harness, extent};
use domengine::{Constraints, Content, Layout};
use graphics::shared::{Backend, Cpu, Frame, Scene, Visual};

fn build(test: &mut Harness) -> Layout {
    test.capture
        .update(&test.document, |input| {
            test.pipeline.build(input, Constraints::viewport(extent()))
        })
        .unwrap()
}
fn frame(layout: &Layout) -> Frame {
    Frame::new(
        Scene::new(layout.extent(), layout.revision(), layout.paint()).unwrap(),
        Visual::new(extent()),
    )
    .unwrap()
}
fn subtree(layout: &Layout, node: usize) -> &[domengine::Placement] {
    let start = layout
        .placements()
        .iter()
        .position(|p| p.local.node == Some(node) && matches!(p.local.content, Content::Box { .. }))
        .unwrap();
    let length = layout.placements()[start + 1..]
        .iter()
        .position(|p| p.parent.is_none_or(|parent| parent < start))
        .unwrap_or(layout.placements().len() - start - 1);
    &layout.placements()[start..start + 1 + length]
}

#[test]
fn sharing() {
    let mut test = Harness::new(
        "<div id='root'><p id='prefix' style='height: 16px;'>Before</p><div id='body' style='padding: 4px; height: 48px; overflow: hidden; opacity: 0.5;'><p>Alpha beta</p><p>Gamma delta</p></div></div>",
    );
    let body = test.document.find("body").unwrap();
    let prefix = test.document.find("prefix").unwrap();
    let original = build(&mut test);
    let retained = original.fragments().to_vec();
    assert_eq!(
        retained,
        original
            .placements()
            .iter()
            .map(|p| p.resolve())
            .collect::<Vec<_>>()
    );
    let pixels = Cpu.render(&frame(&original)).unwrap();
    for height in [48, 0, 32, 16] {
        test.document
            .attribute(prefix, "style", &format!("height: {height}px;"))
            .unwrap();
        let moved = build(&mut test);
        let a = subtree(&original, body.id());
        let b = subtree(&moved, body.id());
        assert_eq!(a.len(), b.len());
        for (before, after) in a.iter().zip(b) {
            assert!(before.local.same(&after.local));
        }
        assert_eq!(b[0].offset[1], height);
        assert_eq!(b[0].local.rect.x, 0);
        assert_eq!(b[0].local.rect.y, 0);
        test.check(Visual::new(extent()));
    }
    test.document
        .append(test.document.find("root").unwrap(), prefix)
        .unwrap();
    let reordered = build(&mut test);
    for (before, after) in subtree(&original, body.id())
        .iter()
        .zip(subtree(&reordered, body.id()))
    {
        assert_ne!(before.id, after.id);
        assert!(before.local.same(&after.local));
    }
    test.pipeline.close();
    drop(test);
    assert_eq!(original.fragments(), retained);
    assert_eq!(Cpu.render(&frame(&original)).unwrap(), pixels);
}

#[test]
fn clipping() {
    let mut test = Harness::new(
        "<div id='outer' style='height: 64px; overflow: hidden;'><div id='body' style='height: 80px;'><p style='height: 80px; background-color: #224466;'>Alpha</p></div></div>",
    );
    let body = test.document.find("body").unwrap();
    let original = build(&mut test);
    test.document
        .attribute(
            test.document.find("outer").unwrap(),
            "style",
            "height: 16px; overflow: hidden;",
        )
        .unwrap();
    let clipped = build(&mut test);
    for (before, after) in subtree(&original, body.id())
        .iter()
        .zip(subtree(&clipped, body.id()))
    {
        assert!(before.local.same(&after.local));
    }
    assert!(original.pick(&frame(&original), 8, 40).is_some());
    assert_eq!(clipped.pick(&frame(&clipped), 8, 40), None);
    assert_eq!(clipped.hit(8, 40), None);
    assert_eq!(subtree(&clipped, body.id())[0].resolve().clip.height, 16);
    test.check(Visual::new(extent()));
}

#[test]
fn invalidation() {
    let mut test = Harness::new("<p id='body' style='width: 96px;'>AB CD EF GH IJ KL MN OP</p>");
    let body = test.document.find("body").unwrap();
    let original = build(&mut test);
    test.document
        .attribute(body, "style", "width: 32px;")
        .unwrap();
    let narrow = build(&mut test);
    assert!(
        !subtree(&original, body.id())[0]
            .local
            .same(&subtree(&narrow, body.id())[0].local)
    );
    assert!(narrow.placements().len() > original.placements().len());
    let expected = narrow.fragments().to_vec();
    let pixels = Cpu.render(&frame(&narrow)).unwrap();
    test.pipeline.enable(false);
    let fresh = build(&mut test);
    assert!(
        !subtree(&narrow, body.id())[0]
            .local
            .same(&subtree(&fresh, body.id())[0].local)
    );
    assert_eq!(fresh.fragments(), expected);
    assert_eq!(Cpu.render(&frame(&fresh)).unwrap(), pixels);
}

#[test]
fn granularity() {
    let mut work = Vec::new();
    for count in [8, 256] {
        let mut test = Harness::new(&format!(
            "<div><p id='prefix' style='height: 16px;'>Before</p><p id='body'>{}</p></div>",
            "<span>A</span>".repeat(count)
        ));
        let body = test.document.find("body").unwrap();
        let original = build(&mut test);
        test.document
            .attribute(
                test.document.find("prefix").unwrap(),
                "style",
                "height: 48px;",
            )
            .unwrap();
        let moved = build(&mut test);
        let a = subtree(&original, body.id());
        let b = subtree(&moved, body.id());
        assert_eq!(a.len(), b.len());
        assert!(a[0].local.same(&b[0].local));
        assert!(test.pipeline.inspect().reused >= count);
        let construction = test.pipeline.inspect().construction;
        work.push((
            construction.requests,
            construction.branches,
            construction.fragments,
        ));
        test.check(Visual::new(extent()));
    }
    assert_eq!(work[0], work[1]);
}

#[test]
fn disposal() {
    use domengine::{Painter, Pipeline};
    use shared::cache::{Manager, Policy};
    let mut test = Harness::new("<div><p>Alpha</p><p>Beta</p></div>");
    let manager = Manager::default();
    let config = config::Store::default();
    let mut pipeline = Pipeline::configured(fixtures::Text, &manager, config.clone()).unwrap();
    let mut painter = Painter::configured(&manager, config.clone()).unwrap();
    let mut build = || {
        test.capture
            .update(&test.document, |input| {
                pipeline.build(input, Constraints::viewport(extent()))
            })
            .unwrap()
    };
    let original = build();
    let pixels = domengine::raster(extent(), &painter.build(&original).unwrap()).unwrap();
    for name in ["layout.fragments", "paint.fragments"] {
        config.set(name, serde_json::to_value(Policy::bounded(0, 0)).unwrap());
        let current = build();
        assert_eq!(
            domengine::raster(extent(), &painter.build(&current).unwrap()).unwrap(),
            pixels
        );
        config.remove(name);
    }
    config.set("layout.fragments", (-1).into());
    assert!(
        test.capture
            .update(&test.document, |input| pipeline
                .build(input, Constraints::viewport(extent())))
            .is_err()
    );
    config.remove("layout.fragments");
    config.set("paint.fragments", (-1).into());
    assert!(painter.build(&original).is_err());
    painter.close();
    pipeline.close();
    assert!(manager.inspect().iter().all(|cache| cache.closed));
    drop(test);
    assert_eq!(
        domengine::raster(extent(), &Layout::paint(&original)).unwrap(),
        pixels
    );
}
#[test]
fn independence() {
    use domengine::{Painter, Pipeline};
    use shared::cache::Policy;
    use std::sync::Arc;
    let mut test = Harness::new(
        "<div id='root'><p id='prefix' style='height: 16px;'>Before</p><p id='body' style='width: 80px; background-color: #224466;'>Alpha beta gamma</p></div>",
    );
    let config = config::Store::default();
    config.set("layout.recurrence", 0.into());
    config.set(
        "layout.fragments",
        serde_json::to_value(Policy::bounded(0, 0)).unwrap(),
    );
    test.pipeline = Pipeline::configured(fixtures::Text, &test.manager, config.clone()).unwrap();
    test.painter = Painter::configured(&test.manager, config).unwrap();
    let original = build(&mut test);
    let retired = Arc::downgrade(original.root());
    let saved = test.painter.build(&original).unwrap();
    let pixels = domengine::raster(extent(), &saved).unwrap();
    let count = original.root().count();
    drop(original);
    let rebuilt = build(&mut test);
    assert!(retired.upgrade().is_none());
    let current = test.painter.build(&rebuilt).unwrap();
    assert_eq!(test.painter.inspect().reused, count);
    assert_eq!(domengine::raster(extent(), &current).unwrap(), pixels);
    assert_eq!(test.pipeline.inspect().reused, 0);
    let resized = test
        .capture
        .update(&test.document, |input| {
            test.pipeline.build(
                input,
                Constraints::viewport(domengine::Extent {
                    width: 64,
                    height: 128,
                }),
            )
        })
        .unwrap();
    assert_eq!(
        domengine::raster(extent(), &test.painter.build(&resized).unwrap()).unwrap(),
        domengine::raster(extent(), &resized.paint()).unwrap()
    );
    test.check(Visual::new(extent()));
    let prefix = test.document.find("prefix").unwrap();
    test.document
        .attribute(prefix, "style", "height: 32px;")
        .unwrap();
    test.check(Visual::new(extent()));
    assert!(test.painter.inspect().reused > 0);
    let body = test.document.find("body").unwrap();
    for style in [
        "width: 32px; background-color: #224466;",
        "width: 32px; background-color: #668822; opacity: 0.5;",
        "width: 96px; translate: 3px 2px;",
    ] {
        test.document.attribute(body, "style", style).unwrap();
        test.check(Visual::new(extent()));
    }
    test.document.replace(body, "Changed text").unwrap();
    test.check(Visual::new(extent()));
    test.document
        .append(test.document.find("root").unwrap(), prefix)
        .unwrap();
    test.check(Visual::new(extent()));
    let cache = test
        .manager
        .inspect()
        .into_iter()
        .rfind(|cache| cache.name == "subsequences")
        .unwrap();
    test.manager
        .command(cache.id, shared::cache::Command::Clear)
        .unwrap();
    test.check(Visual::new(extent()));
    assert_eq!(test.painter.inspect().reused, 0);
    test.painter.close();
    test.pipeline.close();
    drop(test);
    assert_eq!(domengine::raster(extent(), &saved).unwrap(), pixels);
}
