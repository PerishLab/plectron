use super::{check, pixels};
use domengine::{Pipeline, html};
use fixtures::Text;
use shared::cache::{Command, Manager, Policy};

#[test]
fn routing() {
    let mut document =
        html::parse("<p id='text' style='overflow-wrap: anywhere;'>ABCDE</p>").unwrap();
    let manager = Manager::default();
    let config = config::Store::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let original = check(&document, &mut pipeline, 128);
    let saved = pixels(&original);
    check(&document, &mut pipeline, 120);
    assert!(pipeline.inspect().shaping.routed > 0);
    assert_eq!(pipeline.inspect().shaping.materialized, 0);
    check(&document, &mut pipeline, 16);
    assert_eq!(pipeline.inspect().shaping.routed, 0);
    assert!(pipeline.inspect().shaping.materialized > 0);
    check(&document, &mut pipeline, 112);
    assert!(pipeline.inspect().shaping.routed > 0);
    config.set("layout.routing", false.into());
    check(&document, &mut pipeline, 104);
    assert_eq!(pipeline.inspect().shaping.checked, 0);
    assert!(pipeline.inspect().shaping.materialized > 0);
    config.set("layout.routing", true.into());
    document
        .replace(document.find("text").unwrap(), "FGHIJ")
        .unwrap();
    check(&document, &mut pipeline, 96);
    assert_eq!(pipeline.inspect().shaping.routed, 0);
    pipeline.enable(false);
    check(&document, &mut pipeline, 88);
    assert_eq!(pipeline.inspect().shaping.checked, 0);
    config.set("layout.routing", (-1).into());
    let input = domengine::Input::capture(&document, &domengine::Sheet::default()).unwrap();
    assert!(pipeline.build(&input, super::constraints(80)).is_err());
    drop(pipeline);
    assert_eq!(pixels(&original), saved);
}

#[test]
fn boundaries() {
    for source in [
        "<p style='overflow-wrap: anywhere;'>ABCDE</p>",
        "<p style='letter-spacing: 2px; text-align: center;'>A B C</p>",
        "<p style='white-space: pre-wrap;'>A\tB\nC</p>",
        "<p style='direction: rtl; font-family: noto-arabic;'>مرحبا</p>",
    ] {
        let document = html::parse(source).unwrap();
        let mut pipeline = Pipeline::new(Text);
        let first = check(&document, &mut pipeline, 128);
        let saved = pixels(&first);
        for width in [64, 41, 40, 39, 32, 16, 8, 1, 72] {
            check(&document, &mut pipeline, width);
        }
        pipeline.close();
        assert_eq!(pixels(&first), saved);
    }
}

#[test]
fn views() {
    let document = html::parse(&format!(
        "<p style='font-family: lato; font-size: 16px;'>{}</p>",
        "ABCD ".repeat(100)
    ))
    .unwrap();
    let manager = Manager::default();
    let mut pipeline = super::managed(&manager);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "geometry")
        .unwrap();
    let bytes = fixtures::bytes("Lato-Regular.ttf").len() * 2;
    manager
        .command(
            cache.id,
            Command::Configure {
                policy: Policy::bounded(4096, bytes),
            },
        )
        .unwrap();
    for width in [64, 96, 128, 64] {
        check(&document, &mut pipeline, width);
    }
    assert_eq!(pipeline.inspect().computed, 0);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|entry| entry.id == cache.id)
        .unwrap();
    assert!(cache.stats.entries > 3);
    assert_eq!(cache.stats.omitted, 0);
    assert_eq!(cache.stats.evicted, 0);
    assert!(cache.stats.bytes <= bytes);
}

#[test]
fn isolation() {
    let document = html::parse("<div style='font-family: lato;'><p style='white-space: pre-wrap;'>AA\tBB CC\tDD\nEF\tG</p><p style='overflow-wrap: anywhere;'>office affinity á</p><p style='font-family: noto-arabic; direction: rtl; overflow-wrap: anywhere;'>مرحبا بالعالم</p></div>").unwrap();
    let manager = Manager::default();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let first = check(&document, &mut pipeline, 160);
    let original = pixels(&first);
    let caches = manager.inspect();
    let geometry = caches
        .iter()
        .find(|cache| cache.name == "geometry")
        .unwrap();
    let paragraphs = caches
        .iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap();
    let mut snapshots = Vec::new();
    for width in [40, 72, 24, 160, 48, 96, 40] {
        manager.command(geometry.id, Command::Clear).unwrap();
        let layout = check(&document, &mut pipeline, width);
        assert_eq!(pipeline.inspect().shaping.preparations, 0);
        snapshots.push((pixels(&layout), layout));
    }
    manager.command(paragraphs.id, Command::Clear).unwrap();
    manager.command(geometry.id, Command::Clear).unwrap();
    check(&document, &mut pipeline, 72);
    assert!(pipeline.inspect().shaping.preparations > 0);
    pipeline.close();
    assert_eq!(pixels(&first), original);
    for (image, layout) in snapshots {
        assert_eq!(pixels(&layout), image);
    }
}

#[test]
fn widths() {
    let document = html::parse("<div><p style='overflow-wrap: anywhere;'>office affinity 中文 مرحبا</p><p style='white-space: pre-wrap;'>A\tB\nC D</p></div>").unwrap();
    let manager = Manager::default();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let first = check(&document, &mut pipeline, 120);
    let original = pixels(&first);
    for width in [32, 64, 96, 48] {
        check(&document, &mut pipeline, width);
        assert_eq!(pipeline.inspect().shaping.preparations, 0);
        assert!(pipeline.inspect().shaping.materialized > 0);
    }
    assert_eq!(pixels(&first), original);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap();
    assert!(cache.stats.hits > 0);
    assert!(cache.stats.bytes > 0);
    manager
        .command(
            cache.id,
            Command::Configure {
                policy: Policy::bounded(0, 0),
            },
        )
        .unwrap();
    check(&document, &mut pipeline, 88);
    assert!(pipeline.inspect().shaping.preparations > 0);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap();
    assert_eq!(cache.stats.entries, 0);
    assert_eq!(cache.stats.bytes, 0);
    pipeline.close();
    assert!(manager.inspect().iter().all(|cache| cache.closed));
}

#[test]
fn dependencies() {
    let mut document = html::parse("<p id='text'>AA <img id='atom' src='icon.png' style='display: inline; width: 12px; height: 20px;'> CC</p>").unwrap();
    let atom = document.find("atom").unwrap();
    document
        .image(
            atom,
            "icon.png",
            graphics::shared::Raster::rgba(
                domengine::Extent {
                    width: 1,
                    height: 1,
                },
                &[255, 0, 0, 255],
            )
            .unwrap(),
        )
        .unwrap();
    let mut pipeline = Pipeline::new(Text);
    check(&document, &mut pipeline, 120);
    check(&document, &mut pipeline, 64);
    assert_eq!(pipeline.inspect().shaping.preparations, 0);
    document
        .attribute(atom, "style", "display: inline; width: 32px; height: 36px;")
        .unwrap();
    check(&document, &mut pipeline, 64);
    assert!(pipeline.inspect().shaping.preparations > 0);
    let text = document.find("text").unwrap();
    for style in [
        "font-size: 24px;",
        "letter-spacing: 2px;",
        "direction: rtl;",
        "white-space: pre;",
    ] {
        document.attribute(text, "style", style).unwrap();
        check(&document, &mut pipeline, 64);
        assert!(pipeline.inspect().shaping.preparations > 0);
    }
    document.replace(text, "different text").unwrap();
    check(&document, &mut pipeline, 64);
    assert!(pipeline.inspect().shaping.preparations > 0);
    pipeline.enable(false);
    check(&document, &mut pipeline, 80);
    assert!(pipeline.inspect().shaping.preparations > 0);
}

#[test]
fn resources() {
    let document = html::parse(&format!(
        "<div style='font-family: lato;'>{}</div>",
        (0..100)
            .map(|index| format!("<p>AB{index}</p>"))
            .collect::<String>()
    ))
    .unwrap();
    let manager = Manager::default();
    let mut pipeline = Pipeline::managed(Text, &manager).unwrap();
    let id = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap()
        .id;
    let font = fixtures::bytes("Lato-Regular.ttf").len();
    manager
        .command(
            id,
            Command::Configure {
                policy: Policy::bounded(4096, font * 2),
            },
        )
        .unwrap();
    check(&document, &mut pipeline, 120);
    check(&document, &mut pipeline, 96);
    assert_eq!(pipeline.inspect().shaping.preparations, 0);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.id == id)
        .unwrap();
    assert_eq!(cache.stats.resources, cache.stats.entries + 1);
    assert!(cache.stats.shared > font);
    assert!(cache.stats.shared < font * 2);
    assert!(cache.stats.entries >= 100);
    assert_eq!(cache.stats.evicted, 0);
}

#[test]
fn materialization() {
    let document = html::parse("<p>ABCDE</p>").unwrap();
    let manager = Manager::default();
    let config = config::Store::default();
    config.set("layout.payloads", false.into());
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let original = check(&document, &mut pipeline, 128);
    let saved = pixels(&original);
    config.set("layout.materialization", false.into());
    check(&document, &mut pipeline, 120);
    assert_eq!(pipeline.inspect().construction.text.retained, 0);
    assert_eq!(pipeline.inspect().construction.text.generated, 5);
    config.set("layout.materialization", true.into());
    check(&document, &mut pipeline, 112);
    assert_eq!(pipeline.inspect().construction.text.retained, 1);
    let fragments = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "fragments")
        .unwrap();
    manager.command(fragments.id, Command::Clear).unwrap();
    check(&document, &mut pipeline, 104);
    assert_eq!(pipeline.inspect().construction.text.retained, 0);
    assert_eq!(pipeline.inspect().construction.text.generated, 5);
    config.set("layout.materialization", (-1).into());
    let input = domengine::Input::capture(&document, &domengine::Sheet::default()).unwrap();
    assert!(pipeline.build(&input, super::constraints(96)).is_err());
    pipeline.close();
    assert_eq!(pixels(&original), saved);
}
