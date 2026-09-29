use graphics::shared::{
    Backend, Cpu, Extent, Frame, Ink, Item, Raster, Record, Rect, Scene, Visual,
};
use std::sync::Arc;
fn raster() -> Raster {
    Raster::rgba(
        Extent {
            width: 2,
            height: 2,
        },
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 128, 255, 255, 255, 0,
        ],
    )
    .unwrap()
}
fn frame(record: Record) -> Frame {
    let extent = Extent {
        width: 4,
        height: 4,
    };
    Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Draw {
                origin: [0; 2],
                clip: None,
                record: Arc::new(record),
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}
#[test]
fn interpolation() {
    let frame = frame(
        Record::new(Arc::new(Ink::Image {
            raster: raster(),
            rect: Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
        }))
        .unwrap(),
    );
    let image = Cpu.render(&frame).unwrap();
    assert_eq!(image.pixels[0], 0xff0000);
    assert_eq!(image.pixels[1], 0xbf4000);
    assert_eq!(image.pixels[3], 0x00ff00);
    assert_eq!(image.pixels[12], 0x7f7fff);
    assert_eq!(image.pixels[15], 0xffffff);
}
#[test]
fn ownership() {
    let raster = raster();
    let ink = Arc::new(Ink::Image {
        raster: raster.clone(),
        rect: Rect {
            x: 0,
            y: 0,
            width: 4,
            height: 4,
        },
    });
    let record = Record::new(ink).unwrap();
    let serialized = serde_json::to_vec(&record).unwrap();
    let original = frame(record);
    drop(raster);
    let replay = frame(serde_json::from_slice(&serialized).unwrap());
    assert_eq!(Cpu.render(&original).unwrap(), Cpu.render(&replay).unwrap());
    let invalid = r#"{"extent":{"width":1,"height":1},"pixels":[16777215]}"#;
    assert!(serde_json::from_str::<Raster>(invalid).is_err());
    assert!(
        Raster::rgba(
            Extent {
                width: 0,
                height: 1
            },
            &[]
        )
        .is_err()
    );
    assert!(
        Raster::rgba(
            Extent {
                width: 1,
                height: 1
            },
            &[0, 0, 0]
        )
        .is_err()
    );
}
#[test]
fn clipping() {
    use graphics::shared::{Output, Space};
    let raster = raster();
    let extent = Extent {
        width: 12,
        height: 12,
    };
    let record = Arc::new(
        Record::new(Arc::new(Ink::Image {
            raster,
            rect: Rect {
                x: 0,
                y: 0,
                width: 8,
                height: 8,
            },
        }))
        .unwrap(),
    );
    let build = |clip| {
        Frame::new(
            Scene::new(
                extent,
                0,
                vec![Item::Group {
                    space: Space {
                        id: 1,
                        translation: [-2, 1],
                        clip,
                        scroll: None,
                    },
                    opacity: 127,
                    items: vec![Item::Draw {
                        origin: [1, 0],
                        clip: None,
                        record: record.clone(),
                    }]
                    .into(),
                }],
            )
            .unwrap(),
            Visual::new(extent),
        )
        .unwrap()
    };
    let whole = build(None);
    let clipped = build(Some(
        Rect {
            x: 4,
            y: 2,
            width: 3,
            height: 4,
        }
        .into(),
    ));
    for scale in [1.0, 1.5, 2.0] {
        let output = Output::new(extent, scale).unwrap();
        let original = Cpu.submit(&whole.prepare(output).unwrap()).unwrap();
        let partial = Cpu.submit(&clipped.prepare(output).unwrap()).unwrap();
        assert_ne!(original, partial);
        let point = [(3.0 * scale) as usize, (4.0 * scale) as usize];
        let offset = point[1] * partial.extent.width + point[0];
        assert_eq!(original.pixels[offset], partial.pixels[offset]);
        assert_eq!(partial.pixels[0], 0xffffff);
    }
}

#[test]
fn admission() {
    let raster = Raster::rgba(
        Extent {
            width: 1024,
            height: 1024,
        },
        &vec![255; 4_194_304],
    )
    .unwrap();
    let record = Arc::new(
        Record::new(Arc::new(Ink::Image {
            raster,
            rect: Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
        }))
        .unwrap(),
    );
    let extent = Extent {
        width: 4,
        height: 4,
    };
    let scene = Scene::new(
        extent,
        0,
        vec![
            Item::Draw {
                record,
                origin: [0; 2],
                clip: None
            };
            17
        ],
    )
    .unwrap();
    let frame = Frame::new(scene, Visual::new(extent)).unwrap();
    assert!(
        Cpu.render(&frame)
            .unwrap_err()
            .to_string()
            .contains("image work budget")
    );
}

#[test]
fn demand() {
    let extent = Extent {
        width: 4,
        height: 4,
    };
    let raster = Raster::rgba(
        Extent {
            width: 1024,
            height: 1024,
        },
        &vec![255; 4_194_304],
    )
    .unwrap();
    let record = Arc::new(
        Record::new(Arc::new(Ink::Image {
            raster,
            rect: Rect {
                x: 0,
                y: 0,
                width: 4,
                height: 4,
            },
        }))
        .unwrap(),
    );
    let items: Arc<[Item]> = vec![
        Item::Draw {
            record,
            origin: [0; 2],
            clip: None
        };
        17
    ]
    .into();
    for opacity in [0, 255] {
        let scene = Scene::new(
            extent,
            0,
            vec![Item::Group {
                space: graphics::shared::Space {
                    id: 1,
                    translation: [0; 2],
                    clip: None,
                    scroll: None,
                },
                opacity,
                items: items.clone(),
            }],
        )
        .unwrap();
        let frame = Frame::new(scene, Visual::new(extent)).unwrap();
        let result = Cpu.render(&frame);
        let mut preparation = graphics::shared::Preparation::default();
        let prepared =
            preparation.prepare(&frame, graphics::shared::Output::new(extent, 1.0).unwrap());
        assert_eq!(result.is_ok(), prepared.is_ok());
        if opacity == 0 {
            assert!(result.is_ok());
        } else {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("image work budget")
            );
        }
    }
}
