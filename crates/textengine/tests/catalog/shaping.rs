use super::{
    Shaper, request,
    selection::{catalog, equal},
};
use shared::cache::{Command, Manager, Policy, Snapshot};
use textengine::Prepared;

fn snapshot(manager: &Manager) -> Snapshot {
    manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "shaping")
        .unwrap()
}

#[test]
fn equivalence() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let mut request = request(&["lato", "noto-arabic", "noto-sc"]);
    for text in ["Card 383", "office e\u{0301} 中文", "السَّلَامُ عَلَيْكُمْ", ""]
    {
        for (size, spacing) in [(12, 0), (32, 0), (16, 2)] {
            request.size = size.into();
            let selections = catalog.select(text, &request, spacing).unwrap();
            let expected = Prepared::new(text, selections)
                .unwrap()
                .shape(0..text.len())
                .unwrap();
            equal(&expected, &catalog.shape(text, &request, spacing).unwrap());
            manager
                .command(snapshot(&manager).id, Command::Clear)
                .unwrap();
            equal(&expected, &catalog.shape(text, &request, spacing).unwrap());
        }
    }
}

#[test]
fn identity() {
    let manager = Manager::default();
    let catalog = catalog(&manager, &config::Store::default());
    let mut request = request(&["lato"]);
    catalog.shape("office", &request, 0).unwrap();
    let before = snapshot(&manager).stats;
    request.size = 12.into();
    catalog.shape("Changed Card 383", &request, 0).unwrap();
    let after = snapshot(&manager).stats;
    assert_eq!(after.hits, before.hits + 1);
    assert_eq!(after.inserted, before.inserted);
    catalog.shape("office", &request, 2).unwrap();
    assert_eq!(snapshot(&manager).stats.inserted, before.inserted + 1);
    let original = ::fixtures::face("lato", super::Weight::Normal);
    let replacement = serde_json::from_value(serde_json::to_value(original).unwrap()).unwrap();
    catalog
        .register("lato", super::Weight::Normal, replacement)
        .unwrap();
    catalog.shape("office", &request, 0).unwrap();
    assert_eq!(snapshot(&manager).stats.inserted, before.inserted + 2);
}

#[test]
fn lifetime() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let request = request(&["lato"]);
    let prepared = catalog.prepare("office", &request, 0).unwrap();
    let expected = prepared.shape(0..6).unwrap();
    let retained = prepared.cost().total();
    let cache = snapshot(&manager);
    assert!(cache.stats.shared > 0);
    config.set(
        "text.shaping",
        serde_json::to_value(Policy::bounded(0, 0)).unwrap(),
    );
    equal(&expected, &catalog.shape("office", &request, 0).unwrap());
    assert_eq!(snapshot(&manager).stats.bytes, 0);
    assert!(snapshot(&manager).stats.blocked > 0);
    manager.command(cache.id, Command::Destroy).unwrap();
    catalog.maintain();
    assert!(snapshot(&manager).closed);
    equal(&expected, &catalog.shape("office", &request, 0).unwrap());
    catalog.close();
    drop(catalog);
    equal(&expected, &prepared.shape(0..6).unwrap());
    assert_eq!(retained, prepared.cost().total());
}

#[test]
fn policy() {
    let manager = Manager::default();
    let config = config::Store::default();
    let catalog = catalog(&manager, &config);
    let request = request(&["lato"]);
    let expected = catalog.shape("office", &request, 0).unwrap();
    config.set(
        "text.shaping",
        serde_json::to_value(Policy {
            read: false,
            write: false,
            entries: 0,
            bytes: 0,
        })
        .unwrap(),
    );
    let uncached = catalog.prepare("office", &request, 0).unwrap();
    equal(&expected, &uncached.shape(0..6).unwrap());
    assert_eq!(snapshot(&manager).stats.bytes, 0);
    config.remove("text.shaping");
    let cached = catalog.prepare("office", &request, 0).unwrap();
    assert!(cached.cost().total() > uncached.cost().total());
    config.set("text.shaping", true.into());
    assert!(
        catalog
            .shape("office", &request, 0)
            .unwrap_err()
            .0
            .contains("text.shaping")
    );
    assert_eq!(config.get("text.shaping"), Some(true.into()));
}

#[test]
fn failure() {
    let textengine::Face::Owned(owner) = ::fixtures::face("lato", super::Weight::Normal) else {
        panic!("owned fixture font");
    };
    let mut bytes: Vec<u8> = serde_json::from_value(serde_json::to_value(owner).unwrap()).unwrap();
    let count = usize::from(u16::from_be_bytes(bytes[4..6].try_into().unwrap()));
    let table = bytes[12..12 + count * 16]
        .chunks_exact(16)
        .find(|record| &record[..4] == b"GSUB")
        .unwrap();
    let offset = u32::from_be_bytes(table[8..12].try_into().unwrap()) as usize;
    bytes[offset..offset + 2].copy_from_slice(&2u16.to_be_bytes());
    let catalog = textengine::Catalog::new();
    catalog
        .register(
            "broken",
            super::Weight::Normal,
            textengine::Face::Owned(textengine::font::Lease::new(bytes.into()).unwrap()),
        )
        .unwrap();
    let request = request(&["broken"]);
    let expected = Prepared::new("", catalog.select("", &request, 0).unwrap()).unwrap();
    let actual = catalog.prepare("", &request, 0).unwrap();
    assert_eq!(
        expected.shape(0..0).unwrap_err(),
        actual.shape(0..0).unwrap_err()
    );
}
