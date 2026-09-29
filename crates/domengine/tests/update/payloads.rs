use domengine::{Constraints, Extent, Input, Layout, Pipeline, Sheet, html};
use fixtures::Text;
use shared::cache::{Command, Manager};

#[test]
fn rebinding() {
    let children = (0..64)
        .map(|index| {
            format!(
                "<p style='color: {};'>Alpha beta gamma delta epsilon.</p>",
                if index % 2 == 0 { "#ff0000" } else { "#0000ff" }
            )
        })
        .collect::<String>();
    let document = html::parse(&format!("<div>{children}</div>")).unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let config = config::Store::default();
    config.set("layout.materialization", false.into());
    let manager = Manager::default();
    let mut engine = Pipeline::configured(Text, &manager, config.clone()).unwrap();
    let mut previous = None;
    for width in [80, 96, 64, 80] {
        let constraints = Constraints::viewport(Extent { width, height: 128 });
        let reference = Layout::build(&input, constraints, &Text).unwrap();
        let actual = engine.build(&input, constraints).unwrap();
        assert_eq!(actual.fragments(), reference.fragments());
        assert!(engine.inspect().construction.text.cached > 0);
        assert!(engine.inspect().construction.text.generated < 64 * 30);
        for point in [(1, 1), (3, 33), (50, 110)] {
            assert_eq!(
                actual.hit(point.0, point.1),
                reference.hit(point.0, point.1)
            );
        }
        previous = Some(actual);
    }
    let old = previous.unwrap();
    let fragments = old.fragments().to_vec();
    let cache = manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "glyphs")
        .unwrap();
    manager.command(cache.id, Command::Clear).unwrap();
    config.set("layout.payloads", false.into());
    let constraints = Constraints::viewport(Extent {
        width: 72,
        height: 128,
    });
    let actual = engine.build(&input, constraints).unwrap();
    assert_eq!(engine.inspect().construction.text.cached, 0);
    assert_eq!(
        actual.fragments(),
        Layout::build(&input, constraints, &Text)
            .unwrap()
            .fragments()
    );
    engine.close();
    assert_eq!(old.fragments(), fragments);
}

#[test]
fn recording() {
    let document = html::parse(&format!("<div>{}</div>",
        "<p style='color: #ff0000;'>Alpha beta gamma</p><p style='color: #0000ff;'>Alpha beta gamma</p>".repeat(8))).unwrap();
    let input = Input::capture(&document, &Sheet::default()).unwrap();
    let layout = Pipeline::new(Text)
        .build(
            &input,
            Constraints::viewport(Extent {
                width: 80,
                height: 128,
            }),
        )
        .unwrap();
    let expected = graphics::shared::raster(layout.extent(), &layout.paint()).unwrap();
    for enabled in [false, true] {
        let config = config::Store::default();
        config.set("paint.identity", enabled.into());
        let manager = Manager::default();
        let mut painter = domengine::Painter::configured(&manager, config).unwrap();
        let items = painter.build(&layout).unwrap();
        assert_eq!(
            graphics::shared::raster(layout.extent(), &items).unwrap(),
            expected
        );
        assert_eq!(painter.inspect().identified > 0, enabled);
        let identities = manager
            .inspect()
            .into_iter()
            .find(|cache| cache.name == "records")
            .unwrap();
        manager.command(identities.id, Command::Clear).unwrap();
        let subsequences = manager
            .inspect()
            .into_iter()
            .find(|cache| cache.name == "subsequences")
            .unwrap();
        manager.command(subsequences.id, Command::Clear).unwrap();
        assert_eq!(
            graphics::shared::raster(layout.extent(), &painter.build(&layout).unwrap()).unwrap(),
            expected
        );
        painter.close();
        assert_eq!(
            graphics::shared::raster(layout.extent(), &items).unwrap(),
            expected
        );
    }
}
