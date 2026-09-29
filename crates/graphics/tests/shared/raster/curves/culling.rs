use graphics::shared::{
    Backend, Cpu, Evaluation, Extent, Font, Frame, Glyph, Ink, Item, Output, Pass, Preparation,
    Rasterization, Record, Rect, Scene, Space, Visual,
};
use shared::cache::{Command, Policy};
use std::sync::Arc;

fn record(position: [i64; 2]) -> Arc<Record> {
    let font = Font {
        face: fixtures::face("lato", graphics::shared::Weight::Normal),
        size: 32.into(),
        ..Font::default()
    };
    Arc::new(
        Record::new(Arc::new(Ink::Text {
            glyphs: vec![Glyph {
                id: font.index('g'),
                x: position[0].into(),
                y: position[1].into(),
            }]
            .into(),
            font,
            color: 0x304050,
        }))
        .unwrap(),
    )
}
fn frame(items: Vec<Item>, offset: usize) -> Frame {
    let extent = Extent {
        width: 80,
        height: 48,
    };
    let clip = Rect {
        x: 0,
        y: 0,
        width: 80,
        height: 48,
    };
    let scene = Scene::new(
        extent,
        0,
        vec![Item::Group {
            space: Space {
                id: 9,
                translation: [0, 0],
                clip: Some(clip.into()),
                scroll: Some(10240),
            },
            opacity: 180,
            items: vec![Item::Group {
                space: Space {
                    id: 2,
                    translation: [0, 0],
                    clip: Some(clip.into()),
                    scroll: None,
                },
                opacity: 255,
                items: items.into(),
            }]
            .into(),
        }],
    )
    .unwrap();
    let mut visual = Visual::new(extent);
    visual.offsets.insert(9, offset);
    Frame::new(scene, visual).unwrap()
}
fn rows(record: &Arc<Record>, offset: usize) -> Frame {
    frame(
        (0..256)
            .map(|row| Item::Draw {
                origin: [4, row * 40],
                clip: None,
                record: record.clone(),
            })
            .collect(),
        offset,
    )
}
fn disabled() -> Policy {
    Policy {
        read: false,
        write: false,
        entries: 0,
        bytes: 0,
    }
}
fn compare(preparation: &mut Preparation, frame: &Frame, scale: f64) -> Pass {
    let output = Output::new(frame.extent(), scale).unwrap();
    let actual = preparation.prepare(frame, output).unwrap();
    let mut reference = Preparation::default();
    reference.evaluation(Evaluation::Quadratic);
    reference.rasterization(Rasterization::Local);
    reference
        .culling(Command::Configure { policy: disabled() })
        .unwrap();
    let expected = reference.prepare(frame, output).unwrap();
    assert_eq!(Cpu.submit(&actual).unwrap(), Cpu.submit(&expected).unwrap());
    actual
}
fn preparation(config: config::Store) -> Preparation {
    let mut preparation = Preparation::configured(config);
    preparation.evaluation(Evaluation::Quadratic);
    preparation.rasterization(Rasterization::Local);
    preparation
}

#[test]
fn scrolling() {
    let record = record([0, 0]);
    let mut preparation = preparation(config::Store::default());
    for offset in [0, 40, 400, 8000] {
        compare(&mut preparation, &rows(&record, offset), 1.25);
        let work = preparation.work();
        assert_eq!(work.records, 256);
        assert_eq!(work.measured, u64::from(offset == 0));
        assert!(work.excluded >= 253);
        assert!(work.outlines <= 4);
        assert_eq!(work.paths + work.skipped, 256);
    }
    assert_eq!(preparation.bounds().stats.entries, 1);
    assert_eq!(preparation.bounds().stats.resources, 1);
}

#[test]
fn overhang() {
    let record = record([-24, -40]);
    let mut preparation = preparation(config::Store::default());
    for scale in [0.25, 0.3, 1.0, 1.25, 2.0, 7.9999] {
        for clip in [
            None,
            Some(Rect {
                x: 68,
                y: 30,
                width: 4,
                height: 16,
            }),
        ] {
            let current = frame(
                vec![Item::Draw {
                    origin: [90, 60],
                    clip: clip.map(Into::into),
                    record: record.clone(),
                }],
                0,
            );
            let actual = compare(&mut preparation, &current, scale);
            assert_eq!(preparation.work().excluded, 0);
            if clip.is_none() {
                assert!(
                    Cpu.submit(&actual)
                        .unwrap()
                        .pixels
                        .iter()
                        .any(|pixel| *pixel != 0)
                );
            }
        }
    }
    assert_eq!(preparation.bounds().stats.entries, 6);
}

#[test]
fn lifecycle() {
    let record = record([0, 0]);
    let config = config::Store::default();
    let mut preparation = preparation(config.clone());
    let current = rows(&record, 0);
    let retained = compare(&mut preparation, &current, 1.0);
    let pixels = Cpu.submit(&retained).unwrap();
    config.set("render.bounds", serde_json::to_value(disabled()).unwrap());
    compare(&mut preparation, &current, 1.0);
    assert_eq!(preparation.work().skipped, 0);
    assert_eq!(preparation.bounds().stats.bytes, 0);
    config.set("render.bounds", (-1).into());
    let output = Output::new(current.extent(), 1.0).unwrap();
    assert!(preparation.prepare(&current, output).is_err());
    config.remove("render.bounds");
    compare(&mut preparation, &current, 1.0);
    assert_eq!(preparation.work().measured, 1);
    preparation.culling(Command::Clear).unwrap();
    assert_eq!(preparation.bounds().stats.resources, 0);
    compare(&mut preparation, &current, 1.0);
    assert_eq!(preparation.work().measured, 1);
    let replacement = Arc::new(Record::new(Arc::new(record.ink().clone())).unwrap());
    compare(&mut preparation, &rows(&replacement, 0), 1.0);
    assert_eq!(preparation.work().measured, 1);
    assert_eq!(preparation.bounds().stats.entries, 2);
    assert_eq!(preparation.bounds().stats.resources, 1);
    preparation.culling(Command::Destroy).unwrap();
    compare(&mut preparation, &current, 1.0);
    assert_eq!(preparation.work().skipped, 0);
    assert!(preparation.bounds().closed);
    drop(preparation);
    assert_eq!(Cpu.submit(&retained).unwrap(), pixels);
}

fn placed(record: &Arc<Record>, origin: usize, translation: i64) -> Frame {
    let extent = Extent {
        width: 80,
        height: 48,
    };
    let scene = Scene::new(
        extent,
        0,
        vec![Item::Group {
            space: Space {
                id: 1,
                translation: [translation, 0],
                clip: None,
                scroll: None,
            },
            opacity: 255,
            items: vec![Item::Draw {
                origin: [origin, 0],
                clip: None,
                record: record.clone(),
            }]
            .into(),
        }],
    )
    .unwrap();
    Frame::new(scene, Visual::new(extent)).unwrap()
}

#[test]
fn fallback() {
    let mut preparation = preparation(config::Store::default());
    let reference = record([0, 0]);
    let expected = compare(&mut preparation, &placed(&reference, 4, 0), 1.25);
    let pixels = Cpu.submit(&expected).unwrap();
    assert!(pixels.pixels.iter().any(|pixel| *pixel != 0));
    for (position, origin, translation) in [(1_048_577, 0, -1_048_573), (-1_048_577, 1_048_581, 0)]
    {
        let current = placed(&record([position, 0]), origin, translation);
        let actual = compare(&mut preparation, &current, 1.25);
        assert_eq!(Cpu.submit(&actual).unwrap(), pixels);
        assert_eq!(preparation.work().measured, 0);
        assert_eq!(preparation.work().excluded, 0);
    }
    let current = placed(&reference, 17_000_000, 0);
    let output = Output::new(current.extent(), 1.25).unwrap();
    assert!(
        preparation
            .prepare(&current, output)
            .unwrap_err()
            .0
            .contains("coordinate budget")
    );
    assert_eq!(preparation.bounds().stats.entries, 1);
    let edge = record([1_048_576, 0]);
    compare(&mut preparation, &placed(&edge, 0, -1_048_576), 8.0);
    let current = placed(&edge, 1_048_576, 1_048_576);
    let output = Output::new(current.extent(), 8.0).unwrap();
    assert!(
        preparation
            .prepare(&current, output)
            .unwrap_err()
            .0
            .contains("coordinate budget")
    );
    assert_eq!(preparation.work().measured, 0);
    assert_eq!(preparation.work().excluded, 0);
}
