use super::compare;
use graphics::metal::Metal;
use graphics::shared::{
    Backend, Cpu, Extent, Frame, Ink, Item, Output, Path, Queue, Record, Scene, Segment, Space,
    Transform, Visual,
};
use std::{sync::Arc, time::Duration};

#[test]
fn example() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let mut page = webengine::Page::new("app://typography/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("typography")).unwrap();
    let frame = page.frame().unwrap();
    page.acknowledge(frame.id()).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    for scale in [1.25, 2.0, 3.0] {
        let output = Output::new(extent, scale).unwrap();
        let pass = frame.prepare(output).unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        let target = queue.output(output).unwrap();
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        assert!(page.frame().is_none());
        queue.release(ticket).unwrap();
        queue.close(target).unwrap();
    }
    page.close();
}

#[test]
fn clipping() {
    let extent = Extent {
        width: 5,
        height: 5,
    };
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Line([5.into(), 0.into()]),
        Segment::Line([5.into(), 5.into()]),
        Segment::Line([0.into(), 5.into()]),
        Segment::Close,
    ])
    .unwrap();
    let record = Record::new(Arc::new(Ink::Path {
        path,
        transform: Transform::default(),
        color: 0,
    }))
    .unwrap();
    let frame = Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Group {
                space: Space {
                    id: 1,
                    translation: [0, 0],
                    clip: Some(
                        graphics::shared::Rect {
                            x: 1,
                            y: 1,
                            width: 2,
                            height: 2,
                        }
                        .into(),
                    ),
                    scroll: None,
                },
                opacity: 128,
                items: vec![Item::Draw {
                    origin: [0; 2],
                    clip: None,
                    record: Arc::new(record),
                }]
                .into(),
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap();
    let output = Output::new(extent, 1.25).unwrap();
    let expected = Cpu.submit(&frame.prepare(output).unwrap()).unwrap();
    assert_eq!(expected.pixels[0], 0xffffff);
    assert_eq!(expected.pixels[2 * 7 + 2], 0x7f7f7f);
    assert_eq!(expected.pixels[7 + 1], 0xb7b7b7);
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.output(output).unwrap();
    let ticket = queue.submit(target, &frame).unwrap();
    queue.wait(ticket, Duration::from_secs(3)).unwrap();
    compare(&queue.read(ticket).unwrap(), &expected);
    queue.release(ticket).unwrap();
    queue.close(target).unwrap();
}

#[test]
fn bands() {
    let mut segments = vec![
        Segment::Move([(-100).into(), (-100).into()]),
        Segment::Line([1000.into(), (-100).into()]),
        Segment::Line([1000.into(), 1000.into()]),
        Segment::Line([(-100).into(), 1000.into()]),
        Segment::Close,
    ];
    for row in (8..128).step_by(16) {
        segments.extend([
            Segment::Move([8.into(), row.into()]),
            Segment::Quad(
                [(-8).into(), (row + 12).into()],
                [32.into(), (row + 12).into()],
            ),
            Segment::Quad([64.into(), row.into()], [8.into(), row.into()]),
            Segment::Close,
        ]);
    }
    let record = Arc::new(
        Record::new(Arc::new(Ink::Path {
            path: Path::new(segments).unwrap(),
            transform: Transform::default().translate([
                graphics::shared::Scalar::new(0.375),
                graphics::shared::Scalar::new(-0.125),
            ]),
            color: 0x224466,
        }))
        .unwrap(),
    );
    let extent = Extent {
        width: 64,
        height: 144,
    };
    let frame = Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Draw {
                origin: [0; 2],
                clip: Some(
                    graphics::shared::Rect {
                        x: 3,
                        y: 5,
                        width: 48,
                        height: 128,
                    }
                    .into(),
                ),
                record,
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    for scale in [1.0, 1.25, 2.0] {
        let output = Output::new(extent, scale).unwrap();
        let expected = Cpu.submit(&frame.prepare(output).unwrap()).unwrap();
        let target = queue.output(output).unwrap();
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        queue.release(ticket).unwrap();
        queue.close(target).unwrap();
    }
}
