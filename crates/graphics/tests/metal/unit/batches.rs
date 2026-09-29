use crate::metal::{Metal, Resource, Work};
use crate::shared::{
    Backend, Cpu, Extent, Frame, Image, Ink, Item, Output, Path, Preparation, Record, Rect, Scalar,
    Scene, Segment, Space, State, Transform, Visual,
};
use shared::cache::{Command, Policy};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub(super) fn item(origin: [usize; 2], color: u32) -> Item {
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([12.into(), 0.into()], [12.into(), 12.into()]),
        Segment::Quad([0.into(), 12.into()], [0.into(), 0.into()]),
        Segment::Close,
    ])
    .unwrap();
    Item::Draw {
        origin,
        clip: None,
        record: Arc::new(
            Record::new(Arc::new(Ink::Path {
                path,
                transform: Transform::default().translate([Scalar::new(0.375), Scalar::new(0.25)]),
                color,
            }))
            .unwrap(),
        ),
    }
}
pub(super) fn frame(items: Vec<Item>) -> Frame {
    let extent = Extent {
        width: 512,
        height: 96,
    };
    Frame::new(Scene::new(extent, 0, items).unwrap(), Visual::new(extent)).unwrap()
}
fn shapes(count: usize, color: u32) -> Vec<Item> {
    (0..count)
        .map(|index| {
            item(
                [index % 32 * 16, index / 32 * 16],
                color ^ (index as u32 * 739),
            )
        })
        .collect()
}
pub(super) fn read(metal: &mut Metal, mut work: Work, expected: &Image) -> Image {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match metal.poll(&mut work).unwrap() {
            State::Complete => break,
            State::Failed(fault) => panic!("{fault}"),
            State::Pending => (),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    let actual = metal.read(&work).unwrap();
    assert_eq!(actual.extent, expected.extent);
    let maximum = actual
        .pixels
        .iter()
        .zip(&expected.pixels)
        .flat_map(|(a, b)| {
            [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255))
        })
        .max()
        .unwrap();
    if maximum != 0 {
        eprintln!("batch maximum channel difference: {maximum}");
    }
    assert!(maximum <= 1);
    actual
}
#[test]
fn instances() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    let original = frame(shapes(129, 0x224466));
    let output = Output::new(original.extent(), 1.0).unwrap();
    let pass = preparation.prepare(&original, output).unwrap();
    let expected = Cpu.submit(&pass).unwrap();
    for limit in [1_usize, 128, 7] {
        metal.batching(limit).unwrap();
        let before = metal.batches();
        let work = metal.submit(&pass).unwrap();
        assert_eq!(
            metal.batches().calls - before.calls,
            129_usize.div_ceil(limit) as u64
        );
        assert_eq!(metal.batches().paths - before.paths, 129);
        read(&mut metal, work, &expected);
    }
    assert!(metal.batching(0).is_err());
    assert!(metal.batching(129).is_err());
}
#[test]
fn ordering() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    let raster = crate::shared::Raster::rgba(
        Extent {
            width: 1,
            height: 1,
        },
        &[200, 80, 40, 128],
    )
    .unwrap();
    let original = frame(vec![
        item([0, 0], 0xff0000),
        item([32, 0], 0x886600),
        item([0, 0], 0x0000ff),
        item([16, 0], 0x00ff00),
        Item::Fill {
            rect: Rect {
                x: 2,
                y: 2,
                width: 4,
                height: 4,
            },
            color: Some(0x666666),
        },
        item([0, 0], 0x334455),
        Item::Draw {
            origin: [0; 2],
            clip: None,
            record: Arc::new(
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
            ),
        },
        Item::Group {
            space: Space {
                id: 1,
                translation: [1, 2],
                clip: Some(
                    Rect {
                        x: 0,
                        y: 0,
                        width: 24,
                        height: 14,
                    }
                    .into(),
                ),
                scroll: None,
            },
            opacity: 128,
            items: shapes(2, 0x886644).into(),
        },
        item([16, 0], 0x888888),
    ]);
    for scale in [1.0, 1.25, 2.0] {
        let pass = preparation
            .prepare(&original, Output::new(original.extent(), scale).unwrap())
            .unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        metal.batching(1).unwrap();
        let work = metal.submit(&pass).unwrap();
        let reference = read(&mut metal, work, &expected);
        metal.batching(128).unwrap();
        let before = metal.batches();
        let work = metal.submit(&pass).unwrap();
        assert!(metal.batches().overlaps > before.overlaps);
        assert_eq!(read(&mut metal, work, &expected), reference);
    }
}
#[test]
fn leases() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    let original = frame(shapes(64, 0));
    let output = Output::new(original.extent(), 1.0).unwrap();
    let pass = preparation.prepare(&original, output).unwrap();
    let first = metal.submit(&pass).unwrap();
    let expected = Cpu.submit(&pass).unwrap();
    assert_eq!(metal.inspect(Resource::Paths).uploads, 1);
    let colored = preparation
        .prepare(&frame(shapes(64, 0xff0000)), output)
        .unwrap();
    let second = metal.submit(&colored).unwrap();
    let colors = Cpu.submit(&colored).unwrap();
    assert_eq!(metal.inspect(Resource::Paths).uploads, 1);
    metal
        .cache(
            Resource::Paths,
            Command::Configure {
                policy: Policy::bounded(1, 33_554_432),
            },
        )
        .unwrap();
    let split = preparation
        .prepare(&frame(shapes(129, 0x002200)), output)
        .unwrap();
    let third = metal.submit(&split).unwrap();
    let many = Cpu.submit(&split).unwrap();
    assert_eq!(metal.resources(Resource::Paths).stats.evicted, 2);
    metal.cache(Resource::Paths, Command::Destroy).unwrap();
    drop((preparation, pass, colored, split));
    read(&mut metal, first, &expected);
    read(&mut metal, second, &colors);
    read(&mut metal, third, &many);
}
#[test]
fn bytes() {
    let segments = (0..8192)
        .flat_map(|_| {
            [
                Segment::Move([0.into(), 0.into()]),
                Segment::Line([1.into(), 0.into()]),
                Segment::Line([1.into(), 512.into()]),
                Segment::Line([0.into(), 512.into()]),
                Segment::Close,
            ]
        })
        .collect();
    let path = Path::new(segments).unwrap();
    let coverages: Vec<_> = (0..3)
        .map(|index| {
            crate::shared::Geometry::new(
                &path,
                Transform::default().translate([(index * 2).into(), 0.into()]),
                Rect {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 512,
                },
            )
            .unwrap()
        })
        .collect();
    let mut batch = crate::metal::buffer::Batch::default();
    batch.push((&coverages[0], 0));
    assert!(batch.boundary(&coverages[1], 128).is_none());
    batch.push((&coverages[1], 0));
    assert!(matches!(
        batch.boundary(&coverages[2], 128),
        Some(crate::metal::buffer::Split::Limit)
    ));
}
