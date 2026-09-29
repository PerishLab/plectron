use graphics::shared::{Area, Backend, Cpu, Extent, Frame, Item, Rect, Scene, Space, Visual};

fn frame(items: Vec<Item>) -> Frame {
    let extent = Extent {
        width: 16,
        height: 16,
    };
    Frame::new(Scene::new(extent, 0, items).unwrap(), Visual::new(extent)).unwrap()
}

#[test]
fn negative() {
    let items = vec![Item::Group {
        space: Space {
            id: 1,
            translation: [8, 8],
            clip: Some(Area {
                x: -4,
                y: -2,
                width: 8,
                height: 6,
            }),
            scroll: None,
        },
        opacity: 128,
        items: vec![Item::Group {
            space: Space {
                id: 2,
                translation: [-6, -4],
                clip: None,
                scroll: None,
            },
            opacity: 255,
            items: vec![Item::Fill {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 12,
                    height: 12,
                },
                color: Some(0xff0000),
            }]
            .into(),
        }]
        .into(),
    }];
    let original = frame(items.clone());
    let decoded = frame(serde_json::from_str(&serde_json::to_string(&items).unwrap()).unwrap());
    let expected = Cpu
        .render(&frame(vec![Item::Fill {
            rect: Rect {
                x: 4,
                y: 6,
                width: 8,
                height: 6,
            },
            color: Some(0xff7f7f),
        }]))
        .unwrap();
    for current in [original, decoded] {
        assert_eq!(Cpu.render(&current).unwrap(), expected);
        let output = graphics::shared::Output::new(current.extent(), 1.0).unwrap();
        let mut preparation = graphics::shared::Preparation::default();
        assert_eq!(
            Cpu.submit(&preparation.prepare(&current, output).unwrap())
                .unwrap(),
            expected
        );
        assert_eq!(
            current.mapping(2).unwrap().clip,
            Rect {
                x: 4,
                y: 6,
                width: 8,
                height: 6
            }
        );
    }
}

#[test]
fn range() {
    let rect = Rect {
        x: usize::MAX,
        y: 0,
        width: 16,
        height: 16,
    };
    let items = vec![Item::Group {
        space: Space {
            id: 1,
            translation: [0, 0],
            clip: Some(rect.into()),
            scroll: None,
        },
        opacity: 255,
        items: vec![Item::Fill {
            rect: Rect {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            color: Some(0),
        }]
        .into(),
    }];
    let encoded = serde_json::to_value(&items).unwrap();
    let decoded = serde_json::from_value(encoded).unwrap();
    let empty = Cpu.render(&frame(vec![])).unwrap();
    for items in [items, decoded] {
        assert_eq!(Cpu.render(&frame(items)).unwrap(), empty);
    }
}
