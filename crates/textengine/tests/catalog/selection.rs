use super::{Catalog, Face, Shaper, Weight, request};
use shared::cache::{Command, Manager, Policy};

pub(super) fn catalog(manager: &Manager, config: &config::Store) -> Catalog {
    let catalog = Catalog::configured(manager, config.clone()).unwrap();
    for name in ["lato", "noto-arabic", "noto-sc"] {
        catalog
            .register(name, Weight::Normal, ::fixtures::face(name, Weight::Normal))
            .unwrap();
    }
    catalog
}

pub(super) fn equal(a: &textengine::Run, b: &textengine::Run) {
    assert_eq!((a.height, a.ascent), (b.height, b.ascent));
    assert_eq!(a.spans.len(), b.spans.len());
    for (a, b) in a.spans.iter().zip(&b.spans) {
        assert_eq!(
            (&a.font, &a.source, &a.glyphs),
            (&b.font, &b.source, &b.glyphs)
        );
    }
    assert_eq!(
        serde_json::to_value((&a.glyphs, &a.clusters, &a.issues)).unwrap(),
        serde_json::to_value((&b.glyphs, &b.clusters, &b.issues)).unwrap(),
    );
}

#[test]
fn equivalence() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let request = request(&["lato", "noto-arabic", "noto-sc"]);
    let id = manager.inspect()[0].id;
    for text in [
        "office e\u{0301} 中文",
        "ب\u{200d}ب العربية",
        "\u{2067}بب\u{2069}",
        "\u{1f1e8}\u{1f1f3}",
        "",
    ] {
        let first = catalog.shape(text, &request, 0).unwrap();
        equal(&first, &catalog.shape(text, &request, 0).unwrap());
        manager.command(id, Command::Clear).unwrap();
        equal(&first, &catalog.shape(text, &request, 0).unwrap());
        config.set(
            "text.selection",
            serde_json::to_value(Policy::bounded(0, 0)).unwrap(),
        );
        equal(&first, &catalog.shape(text, &request, 0).unwrap());
        assert_eq!(manager.inspect()[0].stats.bytes, 0);
        config.remove("text.selection");
    }
    let prepared = catalog.prepare("office", &request, 0).unwrap();
    let first = prepared.shape(0..6).unwrap();
    manager.command(id, Command::Destroy).unwrap();
    catalog.maintain();
    assert!(manager.inspect()[0].closed);
    equal(&first, &catalog.shape("office", &request, 0).unwrap());
    catalog.close();
    equal(&first, &prepared.shape(0..6).unwrap());
    assert!(catalog.shape("office", &request, 0).is_err());
}

#[test]
fn identity() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let single = request(&["lato"]);
    let original = catalog.select("AB", &single, 0).unwrap();
    assert_eq!(manager.inspect()[0].stats.entries, 0);
    let fallback = request(&["lato", "noto-arabic"]);
    catalog.select("AB", &fallback, 0).unwrap();
    let before = manager.inspect()[0].stats.clone();
    catalog.select("BA", &fallback, 0).unwrap();
    let after = manager.inspect()[0].stats.clone();
    assert_eq!(after.hits - before.hits, 2);
    assert_eq!(after.inserted, before.inserted);
    assert!(after.shared > 0);
    let lease = catalog.register("lato", Weight::Normal, Face::Toy).unwrap();
    assert_eq!(manager.inspect()[0].stats.bytes, 0);
    assert_eq!(
        catalog.select("A", &single, 0).unwrap()[0].font.face,
        Face::Toy
    );
    catalog.unregister(&lease).unwrap();
    assert_eq!(catalog.select("AB", &single, 0).unwrap(), original);
    let mut resized = single.clone();
    resized.size = 12.into();
    assert_eq!(
        catalog.select("AB", &resized, 0).unwrap()[0].font.size,
        textengine::Scalar::from(12)
    );
    assert_eq!(catalog.select("AB", &single, 3).unwrap()[0].spacing, 3);
    catalog
        .register("alternate", Weight::Normal, Face::Toy)
        .unwrap();
    for (names, expected) in [
        (["lato", "alternate"], original[0].font.face.clone()),
        (["alternate", "lato"], Face::Toy),
    ] {
        let request = super::request(&names);
        assert_eq!(
            catalog.select("A", &request, 0).unwrap()[0].font.face,
            expected
        );
    }
}

#[test]
fn eviction() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let request = request(&["lato", "noto-arabic"]);
    let expected = catalog.select("AB", &request, 0).unwrap();
    config.set(
        "text.selection",
        serde_json::to_value(Policy::bounded(1, 16_777_216)).unwrap(),
    );
    for _ in 0..2 {
        assert_eq!(catalog.select("AB", &request, 0).unwrap(), expected);
    }
    let cache = &manager.inspect()[0];
    assert_eq!(cache.stats.entries, 1);
    assert!(cache.stats.evicted > 0);
}

#[test]
fn invalid() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let request = request(&["lato"]);
    catalog.select("A", &request, 0).unwrap();
    for value in [config::Value::Bool(true), (-1).into(), "bad".into()] {
        config.set("text.selection", value.clone());
        assert!(
            catalog
                .select("A", &request, 0)
                .unwrap_err()
                .0
                .contains("text.selection")
        );
        assert_eq!(config.get("text.selection"), Some(value));
    }
    config.remove("text.selection");
    assert!(catalog.select("A", &request, 0).is_ok());
}
