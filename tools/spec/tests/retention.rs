use spec::scenario::{self, Options, Report};

fn run(config: config::Store) -> Report {
    scenario::configured(
        "article",
        Options {
            size: "small",
            mode: "--check",
            backend: "cpu",
            platform: ("test", "test"),
        },
        config,
    )
    .unwrap()
}

#[test]
fn multilingual() {
    let retained = run(config::Store::default());
    let config = config::Store::default();
    for (key, entries, bytes) in [
        ("layout.cache", 4096, 8_388_608),
        ("layout.paragraphs", 4096, 8_388_608),
        ("paint.cache", 1024, 4_194_304),
        ("render.bounds", 2048, 4_194_304),
    ] {
        config.set(
            key,
            serde_json::to_value(shared::cache::Policy::bounded(entries, bytes)).unwrap(),
        );
    }
    let constrained = run(config);
    let a = &retained.runs[0];
    let b = &constrained.runs[0];
    assert!(a.closed && b.closed);
    assert!(a.failure.is_none() && b.failure.is_none());
    assert_eq!(a.stages.len(), b.stages.len());
    for (a, b) in a.stages.iter().zip(&b.stages) {
        assert_eq!(a.action, b.action);
        assert_eq!(a.nodes, b.nodes);
        assert_eq!(a.records, b.records);
        assert_eq!(
            a.rendering.as_ref().map(|r| r.checksum),
            b.rendering.as_ref().map(|r| r.checksum)
        );
    }
    let edit = a.stages.iter().find(|s| s.action == "edit").unwrap();
    let repeated = b.stages.iter().find(|s| s.action == "edit").unwrap();
    let retainedparagraphs = edit
        .caches
        .iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap();
    let limitedparagraphs = repeated
        .caches
        .iter()
        .find(|cache| cache.name == "paragraphs")
        .unwrap();
    assert_eq!(retainedparagraphs.stats.oversized, 0);
    assert!(limitedparagraphs.stats.oversized > 0);
    assert!(limitedparagraphs.stats.largest > limitedparagraphs.active.bytes);
    let reorder = a.stages.iter().find(|s| s.action == "reorder").unwrap();
    assert_eq!(reorder.reuse.as_ref().unwrap().shaping.shapes, 0);
    assert_eq!(reorder.painting.as_ref().unwrap().recorded, 0);
    assert_eq!(reorder.rendering.as_ref().unwrap().work.measured, 0);
    let geometry = edit
        .caches
        .iter()
        .find(|cache| cache.name == "geometry")
        .unwrap();
    assert_eq!(geometry.stats.shared, 0);
    assert_eq!(geometry.stats.oversized, 0);
    let cached = edit
        .caches
        .iter()
        .filter(|cache| cache.name == "paint")
        .chain(edit.rendering.as_ref().unwrap().bounds.iter());
    for cache in cached {
        assert_eq!(cache.stats.oversized, 0);
        assert!(cache.stats.largest <= cache.active.bytes);
        assert!(cache.stats.shared >= a.assets["app://scenarios/fonts/SC.ttf"].bytes);
    }
    let omitted = repeated
        .caches
        .iter()
        .filter(|cache| cache.name == "paint")
        .chain(repeated.rendering.as_ref().unwrap().bounds.iter());
    for cache in omitted {
        assert!(cache.stats.oversized > 0);
        assert!(cache.stats.largest > cache.active.bytes);
    }
    for action in ["width", "font"] {
        let stage = a.stages.iter().find(|s| s.action == action).unwrap();
        assert!(stage.reuse.as_ref().unwrap().shaping.shapes > 0);
    }
}
