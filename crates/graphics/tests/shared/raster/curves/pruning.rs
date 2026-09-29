use graphics::shared::{
    Backend, Cpu, Extent, Frame, Item, Output, Preparation, Rect, Scene, Space, Visual,
};
use shared::cache::{Command, Policy};

fn preparation() -> Preparation {
    let config = config::Store::default();
    config.set("render.subtrees", false.into());
    let mut preparation = Preparation::configured(config);
    preparation
        .pruning(Command::Configure {
            policy: Policy::bounded(2048, 16_777_216),
        })
        .unwrap();
    preparation
}
fn scene() -> Scene {
    let extent = Extent {
        width: 16,
        height: 16,
    };
    let mut items = Vec::new();
    for id in 1..=32 {
        items.push(Item::Group {
            space: Space {
                id,
                translation: [0, 0],
                clip: Some(
                    Rect {
                        x: 0,
                        y: 0,
                        width: 16,
                        height: 16,
                    }
                    .into(),
                ),
                scroll: Some(64),
            },
            opacity: 255,
            items: vec![Item::Group {
                space: Space {
                    id: id + 32,
                    translation: [0, 0],
                    clip: None,
                    scroll: None,
                },
                opacity: 128,
                items: vec![Item::Fill {
                    rect: Rect {
                        x: 0,
                        y: 40,
                        width: 8,
                        height: 8,
                    },
                    color: Some(0x123456),
                }]
                .into(),
            }]
            .into(),
        });
    }
    items.push(Item::Fill {
        rect: Rect {
            x: 14,
            y: 14,
            width: 2,
            height: 2,
        },
        color: Some(0xff0000),
    });
    Scene::new(extent, 0, items).unwrap()
}
fn verify(
    preparation: &mut Preparation,
    scene: &Scene,
    visual: Visual,
    scale: f64,
) -> graphics::shared::Image {
    let frame = Frame::new(scene.clone(), visual).unwrap();
    let output = Output::new(frame.extent(), scale).unwrap();
    let actual = Cpu
        .submit(&preparation.prepare(&frame, output).unwrap())
        .unwrap();
    assert_eq!(actual, Cpu.submit(&frame.prepare(output).unwrap()).unwrap());
    actual
}
#[test]
fn invalidation() {
    let scene = scene();
    let mut preparation = preparation();
    let visual = Visual::new(scene.extent());
    let before = verify(&mut preparation, &scene, visual.clone(), 1.0);
    verify(&mut preparation, &scene, visual.clone(), 1.0);
    assert_eq!(preparation.work().reused, 32);
    assert_eq!(preparation.work().branches, 32);
    let mut moved = visual.clone();
    moved.offsets.insert(1, 40);
    let after = verify(&mut preparation, &scene, moved.clone(), 1.0);
    assert_ne!(before, after);
    assert_eq!(preparation.work().reused, 0);
    verify(&mut preparation, &scene, moved, 1.0);
    assert_eq!(preparation.work().reused, 31);
    verify(&mut preparation, &scene, visual.clone(), 1.5);
    assert_eq!(preparation.work().reused, 0);
    verify(&mut preparation, &scene, visual.clone(), 1.5);
    assert_eq!(preparation.work().reused, 32);
    preparation.pruning(Command::Clear).unwrap();
    verify(&mut preparation, &scene, visual.clone(), 1.5);
    assert_eq!(preparation.work().reused, 0);
    preparation
        .pruning(Command::Configure {
            policy: Policy {
                read: false,
                write: false,
                entries: 0,
                bytes: 0,
            },
        })
        .unwrap();
    verify(&mut preparation, &scene, visual, 1.5);
    assert_eq!(preparation.work().reused, 0);
}
#[test]
fn opacity() {
    let mut scene = scene();
    let mut items = scene.items().to_vec();
    let Item::Group { opacity, .. } = &mut items[0] else {
        panic!()
    };
    *opacity = 0;
    scene = Scene::new(scene.extent(), 1, items.clone()).unwrap();
    let mut preparation = preparation();
    let mut visual = Visual::new(scene.extent());
    visual.offsets.insert(1, 40);
    verify(&mut preparation, &scene, visual.clone(), 1.0);
    verify(&mut preparation, &scene, visual.clone(), 1.0);
    let Item::Group { opacity, .. } = &mut items[0] else {
        panic!()
    };
    *opacity = 255;
    let shown = Scene::new(scene.extent(), 2, items).unwrap();
    let image = verify(&mut preparation, &shown, visual, 1.0);
    assert_ne!(image.pixels[0], 0xffffff);
}

#[test]
fn admission() {
    use graphics::shared::{Font, Glyph, Ink, Record};
    use std::sync::Arc;
    let font = Font::default();
    let (id, cells) = ['W', 'M', '中', '@', '#', 'B']
        .into_iter()
        .map(|c| {
            let id = font.index(c);
            let cells = font
                .glyph(id)
                .unwrap()
                .rows
                .iter()
                .map(|row| row.count_ones() as usize)
                .sum::<usize>();
            (id, cells)
        })
        .max_by_key(|(_, cells)| *cells)
        .unwrap();
    let count = 1_048_576 / (cells * 1024) + 1;
    assert!(count < 64);
    let record = Arc::new(
        Record::new(Arc::new(Ink::Text {
            font,
            color: 0,
            glyphs: vec![
                Glyph {
                    id,
                    x: 0.into(),
                    y: 0.into()
                };
                1024
            ]
            .into(),
        }))
        .unwrap(),
    );
    let extent = Extent {
        width: 4,
        height: 4,
    };
    let output = Output::new(extent, 1.0).unwrap();
    let mut preparation = preparation();
    let mut items = Vec::new();
    for id in 1..=count {
        let item = Item::Group {
            space: Space {
                id: id as u64,
                translation: [0, 0],
                clip: None,
                scroll: None,
            },
            opacity: 255,
            items: vec![Item::Draw {
                origin: [1000, 1000],
                clip: None,
                record: record.clone(),
            }]
            .into(),
        };
        let frame = Frame::new(
            Scene::new(extent, 0, vec![item.clone()]).unwrap(),
            Visual::new(extent),
        )
        .unwrap();
        preparation.prepare(&frame, output).unwrap();
        items.push(item);
    }
    let frame = Frame::new(Scene::new(extent, 1, items).unwrap(), Visual::new(extent)).unwrap();
    assert!(
        frame
            .prepare(output)
            .unwrap_err()
            .to_string()
            .contains("drawing budget")
    );
    assert!(
        preparation
            .prepare(&frame, output)
            .unwrap_err()
            .to_string()
            .contains("drawing budget")
    );
    assert!(preparation.work().reused > 0);
}

#[test]
fn resources() {
    use graphics::shared::{Font, Glyph, Ink, Record};
    use std::sync::Arc;
    let font = Font {
        face: fixtures::face("lato", graphics::shared::Weight::Normal),
        size: 24.into(),
        ..Font::default()
    };
    let record = Record::new(Arc::new(Ink::Text {
        glyphs: vec![Glyph {
            id: font.index('g'),
            x: 0.into(),
            y: 0.into(),
        }]
        .into(),
        font,
        color: 0,
    }))
    .unwrap();
    let scene = Scene::new(
        Extent {
            width: 4,
            height: 4,
        },
        0,
        vec![Item::Group {
            space: Space {
                id: 1,
                translation: [0, 0],
                clip: None,
                scroll: None,
            },
            opacity: 255,
            items: vec![Item::Draw {
                origin: [1000, 1000],
                clip: None,
                record: Arc::new(record),
            }]
            .into(),
        }],
    )
    .unwrap();
    let mut preparation = preparation();
    verify(&mut preparation, &scene, Visual::new(scene.extent()), 1.0);
    assert_eq!(preparation.groups().stats.resources, 1);
    preparation.pruning(Command::Clear).unwrap();
    assert_eq!(preparation.groups().stats.resources, 0);
}
