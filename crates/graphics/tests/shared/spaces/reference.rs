use graphics::shared::{Backend, Cpu, Extent, Frame, Item, Rect, Scene, Space, Visual};

pub(super) fn rect(x: usize, y: usize, width: usize, height: usize) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
pub(super) fn fill(rect: Rect, color: u32) -> Item {
    Item::Fill {
        rect,
        color: Some(color),
    }
}
pub(super) fn group(id: u64, opacity: u8, items: Vec<Item>) -> Item {
    Item::Group {
        space: Space {
            id,
            translation: [0, 0],
            clip: None,
            scroll: None,
        },
        opacity,
        items: items.into(),
    }
}
pub(super) fn frame(items: Vec<Item>) -> Frame {
    let extent = Extent {
        width: 16,
        height: 16,
    };
    Frame::new(Scene::new(extent, 1, items).unwrap(), Visual::new(extent)).unwrap()
}
#[test]
fn isolation() {
    let red = fill(rect(0, 0, 12, 12), 0xff0000);
    let blue = fill(rect(4, 4, 12, 12), 0x0000ff);
    let isolated = group(2, 128, vec![group(3, 255, vec![red.clone(), blue.clone()])]);
    let grouped = Cpu
        .render(&frame(vec![group(1, 255, vec![isolated])]))
        .unwrap();
    let individual = Cpu
        .render(&frame(vec![
            group(1, 128, vec![red]),
            group(2, 128, vec![blue]),
        ]))
        .unwrap();
    assert_eq!(grouped.pixels[0], 0xff7f7f);
    assert_eq!(grouped.pixels[5 * 16 + 5], 0x7f7fff);
    assert_eq!(individual.pixels[5 * 16 + 5], 0x7f3fbf);
    assert_eq!(grouped.pixels[15], 0xffffff);
    let nested = Cpu
        .render(&frame(vec![group(
            1,
            128,
            vec![group(2, 128, vec![fill(rect(0, 0, 16, 16), 0)])],
        )]))
        .unwrap();
    assert_eq!(nested.pixels[0], 0xbfbfbf);
    assert_eq!(
        Cpu.render(&frame(vec![group(1, 0, vec![fill(rect(0, 0, 16, 16), 0)])]))
            .unwrap()
            .pixels[0],
        0xffffff
    );
}
#[test]
fn geometry() {
    let items = vec![Item::Group {
        space: Space {
            id: 1,
            translation: [4, 2],
            clip: Some(rect(0, 0, 8, 8).into()),
            scroll: Some(8),
        },
        opacity: 255,
        items: vec![Item::Group {
            space: Space {
                id: 2,
                translation: [-2, 4],
                clip: Some(rect(0, 0, 8, 8).into()),
                scroll: Some(8),
            },
            opacity: 255,
            items: vec![fill(rect(0, 0, 16, 16), 0x123456)].into(),
        }]
        .into(),
    }];
    let initial = frame(items);
    let mut visual = initial.visual().clone();
    visual.offsets.insert(1, 2);
    visual.offsets.insert(2, 3);
    visual.revision = 1;
    let next = Frame::new(initial.scene().clone(), visual).unwrap();
    for mapping in next.mappings() {
        assert_eq!(next.mapping(mapping.id), Some(mapping));
        assert_eq!(next.scene().range(mapping.id, next.extent()), mapping.range);
    }
    let mapped = next.map(2, rect(0, 0, 16, 16)).unwrap();
    assert_eq!(mapped, rect(4, 4, 6, 6));
    let prefetched = next.prefetch(2, rect(0, 0, 16, 16), 2).unwrap();
    assert_eq!(mapped.intersect(prefetched), mapped);
    assert!(prefetched.width > mapped.width);
    assert_eq!(next.scroller(2), Some(2));
    assert_eq!(next.scroller(1), Some(1));
    let image = Cpu.render(&next).unwrap();
    assert_eq!(image.pixels[5 * 16 + 5], 0x123456);
    assert_eq!(image.pixels[3 * 16 + 5], 0xffffff);
    assert_ne!(Cpu.render(&initial).unwrap(), image);
}
#[test]
fn rejection() {
    let extent = Extent {
        width: 16,
        height: 16,
    };
    assert!(Scene::new(extent, 0, vec![group(1, 255, vec![group(1, 255, vec![])])]).is_err());
    assert!(Scene::new(extent, 0, vec![group(0, 255, vec![])]).is_err());
    let original = frame(vec![]);
    let mut visual = original.visual().clone();
    visual.offsets.insert(0, usize::MAX);
    assert!(Frame::new(original.scene().clone(), visual).is_err());
    let mut visual = original.visual().clone();
    visual.offsets.insert(99, 0);
    assert!(Frame::new(original.scene().clone(), visual).is_err());
    let mut item = fill(rect(0, 0, 16, 16), 0);
    for id in 1..35 {
        item = group(id, 128, vec![item]);
    }
    assert!(Scene::new(extent, 0, vec![item]).is_err());
}
#[test]
fn resources() {
    let extent = Extent {
        width: 2048,
        height: 2048,
    };
    let scene = Scene::new(
        extent,
        0,
        (1..=5).map(|id| group(id, 128, vec![])).collect(),
    )
    .unwrap();
    let frame = Frame::new(scene, Visual::new(extent)).unwrap();
    let pass = frame
        .prepare(graphics::shared::Output::new(extent, 1.0).unwrap())
        .unwrap();
    assert_eq!(pass.layer().groups(), 0);
    assert!(
        Cpu.submit(&pass)
            .unwrap()
            .pixels
            .iter()
            .all(|pixel| *pixel == 0xffffff)
    );
}
#[test]
fn traversal() {
    let inner = Item::Group {
        space: Space {
            id: 7,
            translation: [-1, 2],
            clip: Some(rect(0, 0, 4, 4).into()),
            scroll: Some(4),
        },
        opacity: 255,
        items: vec![fill(rect(0, 0, 2, 4), 0x0000ff)].into(),
    };
    let outer = Item::Group {
        space: Space {
            id: 90,
            translation: [2, 1],
            clip: None,
            scroll: None,
        },
        opacity: 255,
        items: vec![inner].into(),
    };
    let original = frame(vec![
        outer,
        group(40, 255, vec![]),
        group(2, 255, vec![fill(rect(9, 1, 3, 3), 0xff0000)]),
        group(1, 0, vec![group(6, 255, vec![fill(rect(0, 0, 16, 16), 0)])]),
        fill(rect(0, 12, 3, 3), 0x00ff00),
    ]);
    let mut preparation = graphics::shared::Preparation::default();
    for offset in [0, 2, 0] {
        let mut visual = original.visual().clone();
        visual.offsets.insert(7, offset);
        let current = Frame::new(original.scene().clone(), visual).unwrap();
        let ids: Vec<_> = current
            .mappings()
            .iter()
            .map(|mapping| mapping.id)
            .collect();
        assert_eq!(ids, [0, 90, 7, 40, 2, 1, 6]);
        assert_eq!(
            current.mapping(7).unwrap().translation,
            [1, 3 - offset as i64]
        );
        assert_eq!(current.mapping(7).unwrap().parent, Some(90));
        assert_eq!(current.scroller(7), Some(7));
        assert_eq!(
            current.scene().mappings(current.visual()).unwrap().len(),
            ids.len()
        );
        let expected = frame(vec![
            fill(rect(1, 3, 2, 4 - offset), 0x0000ff),
            fill(rect(9, 1, 3, 3), 0xff0000),
            fill(rect(0, 12, 3, 3), 0x00ff00),
        ]);
        for scale in [1.0, 1.25] {
            let output = graphics::shared::Output::new(current.extent(), scale).unwrap();
            let actual = preparation.prepare(&current, output).unwrap();
            let reference = expected.prepare(output).unwrap();
            assert_eq!(
                Cpu.submit(&actual).unwrap(),
                Cpu.submit(&reference).unwrap()
            );
        }
    }
    assert_eq!(Cpu.render(&original).unwrap().pixels[6 * 16 + 1], 0x0000ff);
}
#[test]
fn pruning() {
    let extent = Extent {
        width: 512,
        height: 512,
    };
    let records = (0..128)
        .map(|index| {
            let mut item = group(index + 2, 128, vec![fill(rect(0, 0, 32, 32), 0xff0000)]);
            if let Item::Group { space, .. } = &mut item {
                space.translation[1] = index as i64 * 64;
            }
            item
        })
        .collect();
    let scrolling = Item::Group {
        space: Space {
            id: 1,
            translation: [0; 2],
            clip: Some(rect(0, 0, 32, 32).into()),
            scroll: Some(8192),
        },
        opacity: 255,
        items: records,
    };
    let hidden = group(
        200,
        0,
        (0..128)
            .map(|index| group(index + 201, 128, vec![fill(rect(0, 0, 32, 32), 0)]))
            .collect(),
    );
    let scene = Scene::new(extent, 0, vec![scrolling, hidden]).unwrap();
    let mut preparation = graphics::shared::Preparation::default();
    for (offset, groups) in [(0, 1), (64, 1), (32, 0), (8128, 1), (8192, 0)] {
        let mut visual = Visual::new(extent);
        visual.offsets.insert(1, offset);
        let frame = Frame::new(scene.clone(), visual).unwrap();
        assert_eq!(frame.mappings().len(), 259);
        for scale in [1.0, 1.5] {
            let output = graphics::shared::Output::new(extent, scale).unwrap();
            let direct = frame.prepare(output).unwrap();
            let prepared = preparation.prepare(&frame, output).unwrap();
            assert_eq!(direct.layer().groups(), groups);
            assert_eq!(prepared.layer().groups(), groups);
            let image = Cpu.submit(&direct).unwrap();
            assert_eq!(image, Cpu.submit(&prepared).unwrap());
            assert_eq!(
                image.pixels[0],
                if groups == 0 { 0xffffff } else { 0xff7f7f }
            );
            assert_eq!(image.pixels.last(), Some(&0xffffff));
        }
    }
}

#[test]
fn intermediates() {
    let extent = Extent {
        width: 2048,
        height: 2048,
    };
    let items = (1..=5)
        .map(|id| group(id, 128, vec![fill(rect(0, 0, 1, 1), 0)]))
        .collect();
    let frame = Frame::new(Scene::new(extent, 0, items).unwrap(), Visual::new(extent)).unwrap();
    let output = graphics::shared::Output::new(extent, 1.0).unwrap();
    assert!(
        frame
            .prepare(output)
            .unwrap_err()
            .to_string()
            .contains("intermediate pixel budget")
    );
}
