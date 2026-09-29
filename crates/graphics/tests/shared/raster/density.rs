use graphics::shared::{
    Backend, Cpu, Extent, Font, Frame, Glyph, Item, Output, Primitive, Queue, Rect, Scene, Visual,
};
use std::time::Duration;

fn frame() -> Frame {
    let font = Font {
        face: ::fixtures::face("lato", textengine::Weight::Normal),
        size: 32.into(),
        ..Font::default()
    };
    Frame::new(
        Scene::new(
            Extent {
                width: 80,
                height: 48,
            },
            7,
            vec![Item::Text {
                font: font.clone(),
                color: 0x203040,
                clip: None,
                glyphs: vec![Glyph {
                    id: font.index('é'),
                    x: 8.into(),
                    y: 4.into(),
                }],
            }],
        )
        .unwrap(),
        Visual::new(Extent {
            width: 80,
            height: 48,
        }),
    )
    .unwrap()
}

#[test]
fn independent() {
    let frame = frame();
    let scene = frame.scene().id();
    let mapping = frame.map(
        0,
        Rect {
            x: 8,
            y: 4,
            width: 20,
            height: 32,
        },
    );
    let mut edges = Vec::new();
    for scale in [1.0, 1.25, 2.0, 3.0] {
        let output = Output::new(frame.extent(), scale).unwrap();
        let pass = frame.prepare(output).unwrap();
        assert_eq!(pass.id(), frame.id());
        assert_eq!(pass.output().logical(), frame.extent());
        assert_eq!(pass.extent(), output.pixels());
        let Primitive::Path(path, _) = &pass.layer().primitives[0] else {
            panic!("vector glyph expected");
        };
        edges.push(path.edges().len());
        let image = Cpu.submit(&pass).unwrap();
        assert!(image.pixels.iter().any(|pixel| *pixel != 0xffffff));
    }
    assert!(edges[3] > edges[0]);
    assert_eq!(frame.scene().id(), scene);
    assert_eq!(
        frame.map(
            0,
            Rect {
                x: 8,
                y: 4,
                width: 20,
                height: 32
            }
        ),
        mapping
    );
}
#[test]
fn clipping() {
    let extent = Extent {
        width: 3,
        height: 2,
    };
    let frame = Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Fill {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 3,
                    height: 2,
                },
                color: Some(0),
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap();
    let image = Cpu
        .submit(&frame.prepare(Output::new(extent, 1.25).unwrap()).unwrap())
        .unwrap();
    assert_eq!(
        image.extent,
        Extent {
            width: 4,
            height: 3
        }
    );
    assert_eq!(image.pixels[0], 0);
    assert_eq!(image.pixels[3], 0x404040);
    assert_eq!(image.pixels[8], 0x7f7f7f);
    assert_eq!(image.pixels[11], 0x9f9f9f);
}
#[test]
fn lifecycle() {
    let frame = frame();
    let mut queue = Queue::new(Cpu);
    let low = queue
        .output(Output::new(frame.extent(), 1.0).unwrap())
        .unwrap();
    let first = queue.submit(low, &frame).unwrap();
    queue.wait(first, Duration::from_secs(1)).unwrap();
    queue.close(low).unwrap();
    let high = queue
        .output(Output::new(frame.extent(), 2.0).unwrap())
        .unwrap();
    assert_ne!(low, high);
    assert!(queue.submit(low, &frame).is_err());
    let second = queue.submit(high, &frame).unwrap();
    queue.wait(second, Duration::from_secs(1)).unwrap();
    assert_eq!(queue.read(first).unwrap().extent.width, 80);
    assert_eq!(queue.read(second).unwrap().extent.width, 160);
    queue.release(first).unwrap();
    queue.release(second).unwrap();
    queue.close(high).unwrap();
}
#[test]
fn admission() {
    let frame = frame();
    for scale in [0.0, -1.0, f64::NAN, f64::INFINITY, 9.0] {
        assert!(Output::new(frame.extent(), scale).is_err());
    }
    assert!(
        Output::new(
            Extent {
                width: 2048,
                height: 2048
            },
            2.0
        )
        .is_err()
    );
    let output = Output::new(
        Extent {
            width: 79,
            height: 48,
        },
        2.0,
    )
    .unwrap();
    assert!(frame.prepare(output).is_err());
}
