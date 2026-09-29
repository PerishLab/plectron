use domengine::{
    Sheet, html,
    style::{Inherited, Resolver, Subject},
};
use shared::cache::{Command, Manager};
use std::sync::Arc;

#[test]
fn expiration() {
    let mut document = html::parse("<p id='text'>ABCDE</p>").unwrap();
    let manager = Manager::default();
    let mut pipeline = domengine::Pipeline::managed(fixtures::Text, &manager).unwrap();
    super::check(&document, &mut pipeline, 128);
    document
        .replace(document.find("text").unwrap(), "FGHIJ")
        .unwrap();
    super::check(&document, &mut pipeline, 128);
    let caches = manager.inspect();
    let geometry = caches
        .iter()
        .find(|cache| cache.name == "geometry")
        .unwrap();
    let paragraphs = caches
        .iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap();
    let attachments = caches
        .iter()
        .find(|cache| cache.name == "attachments")
        .unwrap();
    manager.command(geometry.id, Command::Clear).unwrap();
    manager.command(attachments.id, Command::Clear).unwrap();
    let mut policy = paragraphs.active;
    policy.write = false;
    manager
        .command(paragraphs.id, Command::Configure { policy })
        .unwrap();
    document
        .replace(document.find("text").unwrap(), "ABCDE")
        .unwrap();
    for width in [120, 112] {
        super::check(&document, &mut pipeline, width);
        let report = pipeline.inspect().shaping;
        assert_eq!(report.preparations, 0);
        assert_eq!(report.routed, 0);
        assert!(report.materialized > 0);
    }
    policy.write = true;
    manager
        .command(paragraphs.id, Command::Configure { policy })
        .unwrap();
    super::check(&document, &mut pipeline, 104);
    super::check(&document, &mut pipeline, 96);
    assert!(pipeline.inspect().shaping.routed > 0);
}

#[test]
fn payloads() {
    let document = html::parse("<p>ABCDE</p>").unwrap();
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("layout.retention", false.into());
    config.set("layout.recurrence", 0.into());
    let mut pipeline = domengine::Pipeline::configured(fixtures::Text, &manager, config).unwrap();
    let first = super::check(&document, &mut pipeline, 128);
    let attachments = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "attachments")
        .unwrap();
    manager.command(attachments.id, Command::Clear).unwrap();
    let second = super::check(&document, &mut pipeline, 128);
    assert!(pipeline.inspect().dependencies > 0);
    assert!(pipeline.inspect().computed > 0);
    assert_eq!(super::pixels(&first), super::pixels(&second));
}

#[test]
fn styles() {
    let manager = Manager::default();
    let document = html::parse("<p id='text'>ABC</p>").unwrap();
    let subject = Subject::capture(&document, document.find("text").unwrap()).unwrap();
    let mut resolver = Resolver::managed(
        Sheet::parse("p { color: #112233; font-family: lato; }").unwrap(),
        &manager,
    )
    .unwrap();
    let original = resolver
        .resolve(subject.clone(), Inherited::default())
        .unwrap();
    let mut changed = resolver
        .resolve(subject.clone(), Inherited::default())
        .unwrap();
    assert!(Arc::ptr_eq(&original, &changed));
    let style = Arc::make_mut(&mut changed);
    style.color = 0xff0000;
    Arc::make_mut(&mut style.typography.font.families)[0] = "toy".into();
    let cached = resolver
        .resolve(subject.clone(), Inherited::default())
        .unwrap();
    assert_eq!(cached.color, 0x112233);
    assert_eq!(cached.typography.font.families.as_ref(), ["lato"]);
    resolver.replace(Sheet::parse("p { color: #445566; font-family: toy; }").unwrap());
    let replacement = resolver.resolve(subject, Inherited::default()).unwrap();
    for cache in manager.inspect() {
        manager.command(cache.id, Command::Destroy).unwrap();
    }
    resolver.close();
    assert_eq!(original.color, 0x112233);
    assert_eq!(original.typography.font.families.as_ref(), ["lato"]);
    assert_eq!(replacement.color, 0x445566);
    assert_eq!(replacement.typography.font.families.as_ref(), ["toy"]);
    assert_eq!(changed.color, 0xff0000);
}

#[test]
fn analysis() {
    use domengine::{Capture, Input, Layout, Pipeline};
    let document =
        html::parse("<div style='overflow-wrap: anywhere;'><p>ABCDE</p><p>FGHIJ</p></div>")
            .unwrap();
    let manager = Manager::default();
    let config = config::Store::default();
    let mut pipeline = Pipeline::configured(fixtures::Text, &manager, config.clone()).unwrap();
    let mut capture = Capture::new(Resolver::new(Sheet::default()));
    let original = capture
        .update(&document, |input| {
            pipeline.build(input, super::constraints(128))
        })
        .unwrap();
    let pixels = super::pixels(&original);
    assert_eq!(pipeline.inspect().construction.analysis.checked, 0);
    for (width, enabled) in [
        (120, true),
        (112, false),
        (104, true),
        (16, true),
        (96, true),
    ] {
        config.set("layout.analysis", enabled.into());
        let actual = capture
            .update(&document, |input| {
                pipeline.build(input, super::constraints(width))
            })
            .unwrap();
        let input = Input::capture(&document, &Sheet::default()).unwrap();
        let expected = Layout::build(&input, super::constraints(width), &fixtures::Text).unwrap();
        assert_eq!(actual.fragments(), expected.fragments());
        assert_eq!(super::pixels(&actual), super::pixels(&expected));
        let work = pipeline.inspect().construction.analysis;
        if !enabled {
            assert_eq!(work.checked, 0);
            assert_eq!(work.stable, 0);
        } else if width == 120 || width == 104 {
            assert_eq!(work.texts, 2);
            assert_eq!(work.stable, 2);
            assert_eq!(work.wrapped, 0);
        } else {
            assert_eq!(work.stable, 0);
            assert_eq!(work.wrapped, 2);
        }
    }
    pipeline.enable(false);
    capture
        .update(&document, |input| {
            pipeline.build(input, super::constraints(88))
        })
        .unwrap();
    assert_eq!(pipeline.inspect().construction.analysis.checked, 0);
    config.set("layout.analysis", (-1).into());
    assert!(
        capture
            .update(&document, |input| pipeline
                .build(input, super::constraints(88)))
            .is_err()
    );
    drop(pipeline);
    assert_eq!(super::pixels(&original), pixels);
}

#[test]
fn residency() {
    let document = html::parse("<div><p>A</p><p>B</p></div>").unwrap();
    let input = domengine::Input::capture(&document, &Sheet::default()).unwrap();
    let layout = domengine::Layout::build(
        &input,
        domengine::Constraints::viewport(domengine::Extent {
            width: 64,
            height: 16,
        }),
        &fixtures::Text,
    )
    .unwrap();
    let initial = layout.residency();
    assert_eq!(initial.tree.strong, 2);
    assert_eq!(initial.snapshot.strong, 1);
    assert_eq!(initial.capture.strong, 1);
    assert_eq!(initial.geometry.strong, 1);
    assert_eq!(initial.branch.strong, 1);
    assert_eq!(initial.records, layout.tree().nodes().len());
    assert_eq!(initial.live, initial.records);
    assert_eq!(initial.states, layout.snapshot().states().count());
    assert_eq!(initial.geometries, layout.snapshot().metrics().count());
    assert!(initial.branches > 0);
    assert!(initial.fragments > 0);
    assert!(initial.bytes.geometry > 0);
    assert!(initial.bytes.projection > 0);
    let lifetime = layout.lifetime();
    let branch = layout.root().clone();
    let snapshot = layout.snapshot().clone();
    assert_eq!(layout.residency().branch.strong, 2);
    drop(input);
    let retained = layout.residency();
    assert_eq!(retained.tree.strong, 1);
    assert_eq!(retained.capture.strong, 1);
    assert_eq!(retained.live, retained.records);
    drop(layout);
    let detached = lifetime.inspect();
    assert_eq!(detached.capture, 1);
    assert_eq!(detached.tree, 1);
    assert_eq!(detached.snapshot, 1);
    assert_eq!(detached.geometry, 0);
    assert_eq!(detached.branch, 1);
    drop(snapshot);
    let projected = lifetime.inspect();
    assert_eq!(projected.capture, 0);
    assert_eq!(projected.tree, 0);
    assert_eq!(projected.snapshot, 0);
    assert_eq!(projected.geometry, 0);
    assert_eq!(projected.branch, 1);
    drop(branch);
    assert_eq!(lifetime.inspect(), domengine::layout::Liveness::default());
}

#[test]
fn pipeline() {
    use graphics::shared::{Backend, Cpu, Frame, Scene, Visual};

    let document = html::parse("<div><p>ABC</p><p>DEF</p></div>").unwrap();
    let manager = Manager::default();
    let input = domengine::Input::capture(&document, &Sheet::default()).unwrap();
    let mut pipeline = domengine::Pipeline::managed(fixtures::Text, &manager).unwrap();
    let mut painter = domengine::Painter::managed(&manager).unwrap();
    let layout = pipeline.build(&input, super::constraints(64)).unwrap();
    let lifetime = layout.lifetime();
    let items = painter.build(&layout).unwrap();
    let scene = Scene::new(layout.extent(), layout.revision(), items).unwrap();
    let frame = Frame::new(scene, Visual::new(layout.extent())).unwrap();
    let pixels = Cpu.render(&frame).unwrap();

    drop(input);
    drop(layout);
    let retained = lifetime.inspect();
    assert!(retained.capture > 0);
    assert!(retained.tree > 0);
    assert!(retained.snapshot > 0);
    assert!(retained.geometry > 0);
    assert!(retained.branch > 0);
    painter.close();
    assert_eq!(lifetime.inspect(), retained);
    pipeline.close();
    assert_eq!(lifetime.inspect(), domengine::layout::Liveness::default());
    assert_eq!(Cpu.render(&frame).unwrap(), pixels);
}
#[test]
fn generation() {
    use domengine::{Capture, Pipeline, style::Resolver};

    let mut document = html::parse("<div><p id='text'>ABC</p><p>DEF</p></div>").unwrap();
    let manager = Manager::default();
    let mut capture = Capture::new(Resolver::managed(Sheet::default(), &manager).unwrap());
    let mut pipeline = Pipeline::managed(fixtures::Text, &manager).unwrap();
    let first = capture
        .update(&document, |input| {
            pipeline.build(input, super::constraints(64))
        })
        .unwrap();
    let lifetime = first.lifetime();
    drop(first);

    document
        .replace(document.find("text").unwrap(), "Changed")
        .unwrap();
    let second = capture
        .update(&document, |input| {
            pipeline.build(input, super::constraints(64))
        })
        .unwrap();
    let stale = lifetime.inspect();
    assert_eq!(stale.capture, 0);
    assert_eq!(stale.tree, 0);
    assert_eq!(stale.snapshot, 0);
    assert_eq!(stale.branch, 0);
    assert!(stale.geometry > 0);

    drop(second);
    capture.close();
    pipeline.close();
    assert_eq!(lifetime.inspect(), domengine::layout::Liveness::default());
}
