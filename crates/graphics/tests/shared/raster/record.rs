#[path = "curves/coordinates.rs"]
mod coordinates;
use graphics::shared::{
    Backend, Cpu, Extent, Font, Frame, Glyph, Ink, Item, Record, Rect, Scene, Visual,
};
use std::sync::Arc;
fn extent() -> Extent {
    Extent {
        width: 64,
        height: 32,
    }
}
fn clip() -> Rect {
    Rect {
        x: 3,
        y: 4,
        width: 24,
        height: 18,
    }
}
fn frame(items: Vec<Item>) -> Frame {
    Frame::new(
        Scene::new(extent(), 1, items).unwrap(),
        Visual::new(extent()),
    )
    .unwrap()
}
#[test]
fn placement() {
    let record = Arc::new(
        Record::new(Arc::new(Ink::Text {
            font: Font::default(),
            glyphs: vec![Glyph {
                id: 128,
                x: 0.into(),
                y: 0.into(),
            }]
            .into(),
            color: 0x223344,
        }))
        .unwrap(),
    );
    for origin in [[0, 0], [4, 6], [20, 12]] {
        let actual = frame(vec![Item::Draw {
            origin,
            clip: Some(clip().into()),
            record: record.clone(),
        }]);
        let expected = frame(vec![Item::Text {
            font: Font::default(),
            glyphs: vec![Glyph {
                id: 128,
                x: (origin[0] as i64).into(),
                y: (origin[1] as i64).into(),
            }],
            clip: Some(clip().into()),
            color: 0x223344,
        }]);
        assert_eq!(Cpu.render(&actual).unwrap(), Cpu.render(&expected).unwrap());
    }
}
#[test]
fn budgets() {
    let glyph = Glyph {
        id: Font::default().index('A'),
        x: 0.into(),
        y: 0.into(),
    };
    assert!(
        Record::new(Arc::new(Ink::Text {
            font: Font::default(),
            glyphs: vec![Glyph {
                id: 999,
                x: 0.into(),
                y: 0.into()
            }]
            .into(),
            color: 0
        }))
        .is_err()
    );
    assert!(
        Record::new(Arc::new(Ink::Fill {
            rect: clip(),
            color: Some(0x1000000)
        }))
        .is_err()
    );
    let record = Arc::new(
        Record::new(Arc::new(Ink::Text {
            font: Font::default(),
            glyphs: vec![glyph.clone(); 32767].into(),
            color: 0,
        }))
        .unwrap(),
    );
    let item = Item::Draw {
        origin: [0, 0],
        clip: Some(clip().into()),
        record,
    };
    assert!(Scene::new(extent(), 1, vec![item.clone(), item.clone()]).is_ok());
    assert!(Scene::new(extent(), 1, vec![item; 3]).is_err());
    let huge = Arc::new(Ink::Text {
        font: Font::default(),
        glyphs: vec![glyph; 65_536].into(),
        color: 0,
    });
    assert!(Record::new(huge).is_err());
}

#[test]
fn transport() {
    let ink = Ink::Text {
        font: Font::default(),
        glyphs: vec![Glyph {
            id: 128,
            x: 0.into(),
            y: 0.into(),
        }]
        .into(),
        color: 0x223344,
    };
    let item = Item::Draw {
        origin: [4, 6],
        clip: Some(clip().into()),
        record: Arc::new(Record::new(Arc::new(ink.clone())).unwrap()),
    };
    let encoded = serde_json::to_string(&item).unwrap();
    let decoded: Item = serde_json::from_str(&encoded).unwrap();
    assert_eq!(
        Cpu.render(&frame(vec![item])).unwrap(),
        Cpu.render(&frame(vec![decoded])).unwrap()
    );
    let value = serde_json::to_value(ink).unwrap();
    for (field, replacement) in [
        ("color", serde_json::json!(0x1000000)),
        ("cost", serde_json::json!(0)),
        ("glyphs", serde_json::json!([{"id": 999, "x": 0, "y": 0}])),
    ] {
        let mut invalid = value.clone();
        invalid[field] = replacement;
        assert!(serde_json::from_value::<Record>(invalid).is_err());
    }
}

#[test]
fn typography() {
    let mut images = Vec::new();
    for (size, weight) in [
        (16, graphics::shared::Weight::Normal),
        (16, graphics::shared::Weight::Bold),
        (23, graphics::shared::Weight::Normal),
    ] {
        let font = Font {
            size: size.into(),
            weight,
            ..Font::default()
        };
        let ink = Ink::Text {
            font: font.clone(),
            glyphs: vec![Glyph {
                id: font.index('A'),
                x: (-3).into(),
                y: (-4).into(),
            }]
            .into(),
            color: 0,
        };
        let record = Record::new(Arc::new(ink)).unwrap();
        let wire = serde_json::to_string(&record).unwrap();
        let decoded: Record = serde_json::from_str(&wire).unwrap();
        assert_eq!(record.ink(), decoded.ink());
        images.push(
            Cpu.render(&frame(vec![Item::Draw {
                origin: [0, 0],
                clip: Some(clip().into()),
                record: Arc::new(decoded),
            }]))
            .unwrap(),
        );
    }
    assert_ne!(images[0], images[1]);
    assert_ne!(images[0], images[2]);
    let count =
        |image: &graphics::shared::Image| image.pixels.iter().filter(|pixel| **pixel == 0).count();
    assert!(count(&images[1]) > count(&images[0]));
    let font = Font {
        size: 4097.into(),
        ..Font::default()
    };
    assert!(
        Record::new(Arc::new(Ink::Text {
            font: font.clone(),
            glyphs: vec![].into(),
            color: 0
        }))
        .is_err()
    );
    assert!(
        Scene::new(
            extent(),
            1,
            vec![Item::Text {
                font: font.clone(),
                glyphs: vec![],
                color: 0,
                clip: Some(clip().into())
            }]
        )
        .is_err()
    );
}

#[test]
fn composition() {
    let font = Font {
        size: 23.into(),
        weight: graphics::shared::Weight::Bold,
        ..Font::default()
    };
    for clipped in [false, true] {
        let local = clipped.then_some(Rect {
            x: 0,
            y: 0,
            width: 20,
            height: 20,
        });
        let expected = Cpu
            .render(&frame(vec![Item::Text {
                font: font.clone(),
                color: 0,
                clip: local.map(|clip| {
                    Rect {
                        x: 12,
                        y: 10,
                        ..clip
                    }
                    .into()
                }),
                glyphs: vec![Glyph {
                    id: font.index('A'),
                    x: 9.into(),
                    y: 6.into(),
                }],
            }]))
            .unwrap();
        for retained in [false, true] {
            let glyphs = vec![Glyph {
                id: font.index('A'),
                x: (-3).into(),
                y: (-4).into(),
            }];
            let item = if retained {
                Item::Draw {
                    origin: [0; 2],
                    clip: None,
                    record: Arc::new(
                        Record::new(Arc::new(Ink::Text {
                            font: font.clone(),
                            glyphs: glyphs.into(),
                            color: 0,
                        }))
                        .unwrap(),
                    ),
                }
            } else {
                Item::Text {
                    font: font.clone(),
                    glyphs,
                    color: 0,
                    clip: None,
                }
            };
            let actual = frame(vec![Item::Group {
                space: graphics::shared::Space {
                    id: 1,
                    translation: [12, 10],
                    clip: local.map(Into::into),
                    scroll: None,
                },
                opacity: 255,
                items: vec![item].into(),
            }]);
            assert_eq!(Cpu.render(&actual).unwrap(), expected);
        }
    }
}
