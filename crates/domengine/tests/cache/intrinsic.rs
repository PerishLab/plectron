use super::{check, pixels};
use domengine::{Input, Layout, Pipeline, Sheet, html};
use fixtures::Text;
use shared::cache::{Command, Manager, Policy};

fn config() -> config::Store {
    let config = config::Store::default();
    config.set(
        "layout.cache",
        serde_json::to_value(Policy::bounded(0, 0)).unwrap(),
    );
    config
}

#[test]
fn constraints() {
    let config = config();
    let manager = Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config).unwrap();
    let mut document = html::parse("<div id='root' style='display: flex;'><p id='child' style='width: 80px;'>A B C</p><p>DE</p></div>").unwrap();
    let first = check(&document, &mut pipeline, 64);
    let original = pixels(&first);
    let work = pipeline.inspect().charged;
    check(&document, &mut pipeline, 64);
    assert!(pipeline.inspect().intrinsic.hits > 0);
    assert_eq!(pipeline.inspect().charged, work);
    for width in [24, 96, 32, 64] {
        check(&document, &mut pipeline, width);
    }
    let root = document.find("root").unwrap();
    document
        .attribute(
            root,
            "style",
            "display: flex; flex-direction: column; height: 80px; align-items: flex-start;",
        )
        .unwrap();
    for width in [24, 96, 32, 64] {
        check(&document, &mut pipeline, width);
    }
    let child = document.find("child").unwrap();
    document
        .replace(child, "Changed text wraps into more lines")
        .unwrap();
    check(&document, &mut pipeline, 64);
    document
        .attribute(
            child,
            "style",
            "width: 48px; font-size: 24px; padding: 3px; min-height: 8px;",
        )
        .unwrap();
    check(&document, &mut pipeline, 64);
    document.remove(root, child).unwrap();
    check(&document, &mut pipeline, 64);
    document.append(root, child).unwrap();
    check(&document, &mut pipeline, 64);
    assert_eq!(pixels(&first), original);
}

#[test]
fn lifecycle() {
    let config = config();
    let manager = Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let document = html::parse("<div style='display: flex;'><p>ABC</p><p>DEF</p></div>").unwrap();
    check(&document, &mut pipeline, 64);
    let id = manager
        .inspect()
        .iter()
        .find(|cache| cache.name == "intrinsic")
        .unwrap()
        .id;
    manager.command(id, Command::Clear).unwrap();
    check(&document, &mut pipeline, 64);
    check(&document, &mut pipeline, 64);
    assert!(pipeline.inspect().intrinsic.hits > 0);
    manager
        .command(
            id,
            Command::Configure {
                policy: Policy::bounded(1, 16_777_216),
            },
        )
        .unwrap();
    check(&document, &mut pipeline, 64);
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.id == id)
        .unwrap();
    assert!(cache.stats.evicted > 0);
    assert_eq!(cache.stats.entries, 1);
    config.set(
        "layout.intrinsic",
        serde_json::to_value(Policy::bounded(0, 0)).unwrap(),
    );
    check(&document, &mut pipeline, 64);
    assert_eq!(pipeline.inspect().intrinsic.hits, 0);
    config.remove("layout.intrinsic");
    check(&document, &mut pipeline, 64);
    manager.command(id, Command::Destroy).unwrap();
    check(&document, &mut pipeline, 64);
    assert_eq!(pipeline.inspect().intrinsic.hits, 0);
    assert!(
        manager
            .inspect()
            .iter()
            .find(|cache| cache.id == id)
            .unwrap()
            .closed
    );
    pipeline.close();
    assert!(manager.inspect().iter().all(|cache| cache.closed));
}

#[test]
fn fonts() {
    let config = config();
    let manager = Manager::default();
    let fonts = fixtures::catalog();
    let mut pipeline = Pipeline::configured(fonts.clone(), &manager, config).unwrap();
    let document =
        html::parse("<div style='display: flex; font-family: lato;'><p>ABC</p><p>DEFG</p></div>")
            .unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let bounds = super::constraints(64);
    let old = pipeline.build(&input, bounds).unwrap();
    let original = pixels(&old);
    pipeline.build(&input, bounds).unwrap();
    assert_eq!(pipeline.inspect().retained, 1);
    assert_eq!(pipeline.inspect().intrinsic.hits, 0);
    let lease = fonts
        .register("lato", textengine::Weight::Normal, textengine::Face::Toy)
        .unwrap();
    for _ in 0..2 {
        let expected = Layout::build(&input, bounds, &fonts).unwrap();
        let actual = pipeline.build(&input, bounds).unwrap();
        assert!(pipeline.inspect().reset);
        assert_eq!(pipeline.inspect().retained, 0);
        assert_eq!(actual.fragments(), expected.fragments());
        fonts.unregister(&lease).unwrap();
    }
    assert_eq!(pixels(&old), original);
}

#[test]
fn invalid() {
    let config = config();
    let manager = Manager::default();
    let mut pipeline = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let document = html::parse("<div style='display: flex;'><p>ABC</p></div>").unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    for value in [config::Value::Null, true.into(), "bad".into()] {
        config.set("layout.intrinsic", value.clone());
        assert!(
            pipeline
                .build(&input, super::constraints(64))
                .unwrap_err()
                .0
                .contains("layout.intrinsic")
        );
        assert_eq!(config.get("layout.intrinsic"), Some(value));
    }
    config.remove("layout.intrinsic");
    let actual = pipeline.build(&input, super::constraints(64)).unwrap();
    let expected = Layout::build(&input, super::constraints(64), &Text).unwrap();
    assert_eq!(actual.fragments(), expected.fragments());
}
