use super::{Harness, extent, records};
use graphics::shared::{Backend, Cpu, Visual};
use shared::cache::{Command, Policy};
use std::sync::Arc;

#[test]
fn policies() {
    let mut test = Harness::new("<p>ABC</p><p>ABC</p>");
    let frame = test.check(Visual::new(extent()));
    let pixels = Cpu.render(&frame).unwrap();
    let retained = records(frame.scene().items());
    assert!(retained.iter().enumerate().any(|(index, record)| {
        retained[index + 1..]
            .iter()
            .any(|other| Arc::ptr_eq(record, other))
    }));
    let id = test
        .manager
        .inspect()
        .into_iter()
        .find(|cache| cache.name == "paint")
        .unwrap()
        .id;
    let disabled = Policy {
        read: false,
        write: false,
        entries: 512,
        bytes: 4_194_304,
    };
    for command in [
        Command::Configure { policy: disabled },
        Command::Configure {
            policy: Policy::bounded(1, 512),
        },
        Command::Clear,
        Command::Configure {
            policy: Policy::bounded(512, 4_194_304),
        },
        Command::Destroy,
    ] {
        test.manager.command(id, command).unwrap();
        let current = test.check(Visual::new(extent()));
        assert_eq!(Cpu.render(&current).unwrap(), pixels);
        let cache = test
            .manager
            .inspect()
            .into_iter()
            .find(|cache| cache.id == id)
            .unwrap();
        assert_eq!(cache.requested, cache.applied);
    }
    assert_eq!(test.painter.inspect().hits, 0);
    assert_eq!(
        test.painter.inspect().requests,
        test.painter.inspect().recorded
    );
    drop(test);
    assert_eq!(Cpu.render(&frame).unwrap(), pixels);
}

#[test]
fn configuration() {
    use domengine::{Constraints, Input, Layout, Painter, Sheet};
    let test = Harness::new("<p>ABC</p><p>XYZ</p>");
    let input = Input::capture(&test.document, &Sheet::default()).unwrap();
    let layout = Layout::build(&input, Constraints::viewport(extent()), &fixtures::Text).unwrap();
    let config = config::Store::default();
    let mut painter = Painter::configured(&test.manager, config.clone()).unwrap();
    let original = painter.build(&layout).unwrap();
    let expected = domengine::raster(extent(), &original).unwrap();
    painter.build(&layout).unwrap();
    assert_eq!(painter.inspect().recorded, 0);
    config.set(
        "paint.cache",
        config::Value::Object(
            [
                ("read".into(), false.into()),
                ("write".into(), false.into()),
                ("entries".into(), 0.into()),
                ("bytes".into(), 0.into()),
            ]
            .into_iter()
            .collect(),
        ),
    );
    let uncached = painter.build(&layout).unwrap();
    assert_eq!(painter.inspect().hits, 0);
    assert_eq!(domengine::raster(extent(), &uncached).unwrap(), expected);
    config.set("paint.cache", (-1).into());
    assert!(painter.build(&layout).is_err());
    assert!(painter.inspect().failure.unwrap().contains("paint.cache"));
    config.remove("paint.cache");
    painter.build(&layout).unwrap();
    painter.build(&layout).unwrap();
    assert_eq!(painter.inspect().recorded, 0);
    let cache = test
        .manager
        .inspect()
        .into_iter()
        .rfind(|cache| cache.name == "paint")
        .unwrap();
    assert_eq!(cache.active.entries, 1024);
    test.manager
        .command(
            cache.id,
            Command::Configure {
                policy: Policy::bounded(0, 0),
            },
        )
        .unwrap();
    painter.build(&layout).unwrap();
    assert_eq!(painter.inspect().hits, 0);
    drop(painter);
    assert_eq!(domengine::raster(extent(), &original).unwrap(), expected);
}
