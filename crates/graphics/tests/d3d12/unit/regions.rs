use crate::d3d12::{D3d12, Work, pool::Policy};
use crate::shared::{
    Backend, Cpu, Extent, Frame, Image, Ink, Item, Output, Path, Raster, Record, Rect, Scene,
    Segment, Space, Transform, Visual,
};
use std::sync::Arc;

fn rect(x: usize, y: usize, width: usize, height: usize) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}
fn group(id: u64, opacity: u8, items: Vec<Item>) -> Item {
    Item::Group {
        space: Space {
            id,
            translation: [3, 5],
            clip: Some(rect(13, 7, 43, 37).into()),
            scroll: None,
        },
        opacity,
        items: items.into(),
    }
}
fn draw(ink: Ink) -> Item {
    Item::Draw {
        origin: [17, 11],
        clip: Some(rect(3, 2, 23, 19).into()),
        record: Arc::new(Record::new(Arc::new(ink)).unwrap()),
    }
}
pub(super) fn frame(color: u32) -> Frame {
    let extent = Extent {
        width: 257,
        height: 123,
    };
    let raster = Raster::rgba(
        Extent {
            width: 2,
            height: 2,
        },
        &[
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 255, 255, 255, 0,
        ],
    )
    .unwrap();
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([27.into(), 0.into()], [27.into(), 25.into()]),
        Segment::Quad([0.into(), 25.into()], [0.into(), 0.into()]),
        Segment::Close,
    ])
    .unwrap();
    let items = vec![
        group(
            1,
            163,
            vec![
                Item::Fill {
                    rect: rect(14, 10, 17, 11),
                    color: Some(color),
                },
                group(
                    2,
                    197,
                    vec![
                        draw(Ink::Path {
                            path,
                            transform: Transform::default(),
                            color: 0x117744,
                        }),
                        draw(Ink::Image {
                            raster,
                            rect: rect(0, 0, 29, 23),
                        }),
                    ],
                ),
            ],
        ),
        group(3, 128, vec![]),
        group(
            4,
            0,
            vec![group(
                5,
                128,
                vec![Item::Fill {
                    rect: rect(14, 10, 32, 24),
                    color: Some(0),
                }],
            )],
        ),
    ];
    Frame::new(Scene::new(extent, 0, items).unwrap(), Visual::new(extent)).unwrap()
}
pub(super) fn read(d3d12: &mut D3d12, work: &Work, expected: &Image) -> Image {
    d3d12.wait(work, std::time::Duration::from_secs(5)).unwrap();
    let actual = d3d12.read(work).unwrap();
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
    eprintln!("cropped groups maximum channel difference: {maximum}");
    assert!(maximum <= 2);
    actual
}
#[test]
fn cropping() {
    let mut d3d12 = D3d12::new().unwrap();
    for scale in [1.0, 1.25, 2.0] {
        let original = frame(0x884422);
        let pass = original
            .prepare(Output::new(original.extent(), scale).unwrap())
            .unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        let mut reference = None;
        for entries in [0, 32] {
            d3d12.pool().clear().unwrap();
            d3d12
                .pool()
                .configure(Policy {
                    entries,
                    bytes: 33_554_432,
                })
                .unwrap();
            let before = d3d12.pool().inspect();
            let work = d3d12.submit(&pass).unwrap();
            let actual = read(&mut d3d12, &work, &expected);
            let after = d3d12.pool().inspect();
            assert_eq!(after.allocations - before.allocations, 2);
            assert!(after.pixels - before.pixels < pass.extent().area().unwrap() as u64 / 8);
            assert_eq!(after.idle.entries, if entries == 0 { 0 } else { 2 });
            if let Some(previous) = &reference {
                assert_eq!(previous, &actual);
            }
            reference = Some(actual);
        }
    }
}
#[test]
fn outputs() {
    let mut d3d12 = D3d12::new().unwrap();
    let first = frame(0xff0000);
    let pass = first
        .prepare(Output::new(first.extent(), 1.0).unwrap())
        .unwrap();
    let work = d3d12.submit(&pass).unwrap();
    let expected = Cpu.submit(&pass).unwrap();
    let saved = read(&mut d3d12, &work, &expected);
    let before = d3d12.pool().inspect();
    let mut retained = Vec::new();
    for color in [0x00ff00, 0x0000ff, 0xffffff, 0x004488] {
        let frame = frame(color);
        let pass = frame
            .prepare(Output::new(frame.extent(), 1.0).unwrap())
            .unwrap();
        let current = d3d12.submit(&pass).unwrap();
        read(&mut d3d12, &current, &Cpu.submit(&pass).unwrap());
        retained.push(current);
        assert_eq!(d3d12.read(&work).unwrap(), saved);
    }
    let after = d3d12.pool().inspect();
    assert_eq!(after.allocations, before.allocations);
    assert_eq!(after.reuses - before.reuses, 8);
    assert_eq!(after.pending.entries, 0);
    d3d12.pool().destroy().unwrap();
    let current = d3d12.submit(&pass).unwrap();
    read(&mut d3d12, &current, &expected);
    assert_eq!(d3d12.pool().inspect().idle.entries, 0);
    assert_eq!(d3d12.read(&work).unwrap(), saved);
    let mut foreign = D3d12::new().unwrap();
    assert!(foreign.read(&work).is_err());
    assert_eq!(foreign.pool().inspect().allocations, 0);
}

#[test]
fn pending() {
    for closed in [false, true] {
        let mut d3d12 = D3d12::new().unwrap();
        let frame = frame(0x664422);
        let pass = frame
            .prepare(Output::new(frame.extent(), 1.25).unwrap())
            .unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        let first = d3d12.submit(&pass).unwrap();
        if closed {
            d3d12.pool().destroy().unwrap();
        } else {
            d3d12.pool().clear().unwrap();
        }
        let second = d3d12.submit(&pass).unwrap();
        let saved = read(&mut d3d12, &first, &expected);
        assert_eq!(read(&mut d3d12, &second, &expected), saved);
        let snapshot = d3d12.pool().inspect();
        assert_eq!(snapshot.allocations, 4);
        assert_eq!(snapshot.reuses, 0);
        assert_eq!(snapshot.pending.entries, 0);
        assert_eq!(snapshot.idle.entries, if closed { 0 } else { 2 });
    }
}
