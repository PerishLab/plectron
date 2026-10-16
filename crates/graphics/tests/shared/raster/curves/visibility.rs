use super::*;
use graphics::shared::{Evaluation, Rasterization, Space};

fn rows(offset: i64, font: &Font) -> Frame {
    let extent = Extent {
        width: 80,
        height: 48,
    };
    Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Group {
                space: Space {
                    id: 1,
                    translation: [0, -offset],
                    clip: None,
                    scroll: None,
                },
                opacity: 180,
                items: vec![Item::Text {
                    font: font.clone(),
                    clip: None,
                    color: 0x304050,
                    glyphs: (0..256)
                        .map(|row| Glyph {
                            id: font.index('g'),
                            x: 4.into(),
                            y: (row * 40).into(),
                        })
                        .collect(),
                }]
                .into(),
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}

#[test]
fn scrolling() {
    let original = frame(4, 0, None);
    let Item::Text { font, .. } = &original.scene().items()[0] else {
        panic!()
    };
    let config = config::Store::default();
    let mut preparation = Preparation::configured(config.clone());
    for offset in [0, 40, 400, 8000] {
        let frame = rows(offset, font);
        let requests = preparation.inspect().stats.requests;
        let pass = verify(&mut preparation, &frame, 1.25);
        assert_eq!(preparation.work().outlines, 256);
        assert!(preparation.work().culled >= 253);
        assert_eq!(preparation.work().decoded, u64::from(offset == 0));
        assert!(preparation.inspect().stats.requests - requests <= 3);
        let expected = Cpu.submit(&pass).unwrap();
        for evaluation in [Evaluation::Linear, Evaluation::Quadratic] {
            preparation.evaluation(evaluation);
            preparation.rasterization(Rasterization::Local);
            let cached = preparation
                .prepare(&frame, Output::new(frame.extent(), 1.25).unwrap())
                .unwrap();
            let mut fresh = Preparation::default();
            fresh.evaluation(evaluation);
            fresh.rasterization(Rasterization::Local);
            let fresh = fresh
                .prepare(&frame, Output::new(frame.extent(), 1.25).unwrap())
                .unwrap();
            assert_eq!(Cpu.submit(&cached).unwrap(), Cpu.submit(&fresh).unwrap());
        }
        preparation.evaluation(Evaluation::Linear);
        preparation.rasterization(Rasterization::Direct);
        assert_eq!(Cpu.submit(&pass).unwrap(), expected);
    }
    config.set(
        "render.outlines",
        serde_json::to_value(Policy::bounded(0, 0)).unwrap(),
    );
    verify(&mut preparation, &rows(0, font), 1.0);
    assert_eq!(preparation.outlines().stats.entries, 0);
    assert_eq!(preparation.work().decoded, 256);
    config.set("render.outlines", (-1).into());
    assert!(
        preparation
            .prepare(
                &rows(0, font),
                Output::new(
                    Extent {
                        width: 80,
                        height: 48
                    },
                    1.0
                )
                .unwrap()
            )
            .is_err()
    );
    config.remove("render.outlines");
    verify(&mut preparation, &rows(0, font), 1.0);
    assert_eq!(preparation.work().decoded, 1);
}

#[test]
fn hull() {
    use graphics::shared::{Ink, Path, Record, Segment, Transform};
    use std::sync::Arc;
    let path = Path::new(vec![
        Segment::Move([100.into(), 0.into()]),
        Segment::Quad([(-100).into(), 24.into()], [100.into(), 48.into()]),
        Segment::Close,
    ])
    .unwrap();
    assert_eq!(
        path.bounds(),
        Some([(-100).into(), 0.into(), 100.into(), 48.into()])
    );
    let encoded = serde_json::to_value(&path).unwrap();
    assert!(encoded.is_array());
    let restored: Path = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored, path);
    let extent = Extent {
        width: 80,
        height: 48,
    };
    for transform in [
        Transform::default(),
        Transform([
            1.into(),
            0.into(),
            (-1).into(),
            1.into(),
            24.into(),
            0.into(),
        ]),
    ] {
        let frame = Frame::new(
            Scene::new(
                extent,
                0,
                vec![Item::Draw {
                    origin: [0; 2],
                    clip: None,
                    record: Arc::new(
                        Record::new(Arc::new(Ink::Path {
                            path: path.clone(),
                            transform,
                            color: 0x103050,
                        }))
                        .unwrap(),
                    ),
                }],
            )
            .unwrap(),
            Visual::new(extent),
        )
        .unwrap();
        let mut preparation = Preparation::default();
        verify(&mut preparation, &frame, 1.25);
        assert_eq!(preparation.work().culled, 0);
    }
}

fn demanded(font: &Font, opacity: u8, offset: i64) -> Frame {
    let source = rows(offset, font);
    let extent = source.extent();
    let items = vec![
        Item::Group {
            space: Space {
                id: 2,
                translation: [0; 2],
                clip: None,
                scroll: None,
            },
            opacity,
            items: source.scene().items().to_vec().into(),
        },
        Item::Group {
            space: Space {
                id: 3,
                translation: [1, 1],
                clip: None,
                scroll: None,
            },
            opacity: 255,
            items: vec![Item::Fill {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                },
                color: Some(0xff0000),
            }]
            .into(),
        },
    ];
    Frame::new(Scene::new(extent, 0, items).unwrap(), Visual::new(extent)).unwrap()
}

#[test]
fn demand() {
    let original = frame(4, 0, None);
    let Item::Text { font, .. } = &original.scene().items()[0] else {
        panic!()
    };
    let mut preparation = Preparation::default();
    let shown = demanded(font, 255, 0);
    let before = verify(&mut preparation, &shown, 1.0);
    let pixels = Cpu.submit(&before).unwrap();
    for offset in [0, 40, 8000] {
        let hidden = demanded(font, 0, offset);
        let outlines = preparation.outlines().stats;
        let requests = outlines.requests;
        let pass = verify(&mut preparation, &hidden, 1.0);
        assert_eq!(preparation.work().undemanded, 2);
        assert_eq!(preparation.work().outlines, 0);
        assert_eq!(preparation.work().paths, 0);
        assert_eq!(preparation.outlines().stats.requests, requests);
        assert_eq!(preparation.outlines().stats.resources, outlines.resources);
        let image = Cpu.submit(&pass).unwrap();
        assert_eq!(image.pixels[image.extent.width + 1], 0xff0000);
        assert_eq!(
            image
                .pixels
                .iter()
                .filter(|pixel| **pixel != 0xffffff)
                .count(),
            1
        );
        preparation.command(shared::cache::Command::Clear).unwrap();
        let restored = demanded(font, 255, offset);
        verify(&mut preparation, &restored, 1.0);
        assert_eq!(preparation.work().undemanded, 0);
        assert_eq!(preparation.work().outlines, 256);
    }
    assert_eq!(Cpu.submit(&before).unwrap(), pixels);
    let mut invalid = shown.scene().items().to_vec();
    let Item::Group { space, opacity, .. } = &mut invalid[0] else {
        panic!()
    };
    space.id = 1;
    *opacity = 0;
    assert!(Scene::new(shown.extent(), 0, invalid).is_err());
}

#[test]
#[ignore = "timing observation, not a correctness check"]
fn observation() {
    let original = frame(4, 0, None);
    let Item::Text { font, .. } = &original.scene().items()[0] else {
        panic!()
    };
    let mut preparation = Preparation::default();
    let mut observations = Vec::new();
    const SAMPLES: usize = 30;
    const WARMUPS: usize = 3;
    for sample in 0..SAMPLES + WARMUPS {
        let modes = if sample.is_multiple_of(2) {
            [0, 255]
        } else {
            [255, 0]
        };
        for opacity in modes {
            let start = std::time::Instant::now();
            let frame = demanded(font, opacity, 0);
            let publication = start.elapsed().as_nanos();
            let start = std::time::Instant::now();
            let pass = preparation
                .prepare(&frame, Output::new(frame.extent(), 1.0).unwrap())
                .unwrap();
            let elapsed = start.elapsed().as_nanos();
            if sample < WARMUPS {
                continue;
            }
            observations.push(serde_json::json!({"sample":sample,"opacity":opacity,"publication":publication,"preparation":elapsed,"work":preparation.work(),"groups":pass.layer().groups()}));
        }
    }
    println!(
        "{}",
        serde_json::json!({"unit":"nanoseconds","samples":SAMPLES,"warmups":WARMUPS,"glyphs":256,"endpoint":"scene/frame construction and Preparation::prepare measured separately; different output demands, not equivalent-work speed ratio","observations":observations})
    );
}
