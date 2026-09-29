use graphics::shared::{
    Backend, Cpu, Extent, Font, Frame, Glyph, Item, Output, Pass, Preparation, Primitive, Queue,
    Rect, Scene, Visual,
};
use shared::cache::{Command, Policy};

fn frame(position: i64, color: u32, clip: Option<Rect>) -> Frame {
    let font = Font {
        face: fixtures::face("lato", graphics::shared::Weight::Normal),
        size: 32.into(),
        ..Font::default()
    };
    let extent = Extent {
        width: 80,
        height: 48,
    };
    Frame::new(
        Scene::new(
            extent,
            0,
            vec![Item::Text {
                glyphs: vec![Glyph {
                    id: font.index('g'),
                    x: position.into(),
                    y: 4.into(),
                }],
                font,
                color,
                clip: clip.map(Into::into),
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}
fn id(pass: &Pass) -> u64 {
    let Primitive::Path(coverage, _) = &pass.layer().primitives[0] else {
        panic!("path");
    };
    coverage.id()
}
fn verify(preparation: &mut Preparation, frame: &Frame, scale: f64) -> Pass {
    let output = Output::new(frame.extent(), scale).unwrap();
    let pass = preparation.prepare(frame, output).unwrap();
    assert_eq!(
        Cpu.submit(&pass).unwrap(),
        Cpu.submit(&frame.prepare(output).unwrap()).unwrap()
    );
    pass
}
#[test]
fn keys() {
    let mut preparation = Preparation::default();
    let original = frame(4, 0, None);
    let first = verify(&mut preparation, &original, 1.0);
    let changed = verify(&mut preparation, &frame(4, 0xff0000, None), 1.0);
    assert_eq!(id(&first), id(&changed));
    assert_eq!(preparation.inspect().stats.hits, 1);
    let shifted = verify(&mut preparation, &frame(5, 0, None), 1.0);
    assert_ne!(id(&first), id(&shifted));
    let scaled = verify(&mut preparation, &original, 1.25);
    assert_ne!(id(&first), id(&scaled));
    let clip = Some(Rect {
        x: 5,
        y: 7,
        width: 10,
        height: 17,
    });
    let clipped = verify(&mut preparation, &frame(4, 0, clip), 1.0);
    assert_ne!(id(&first), id(&clipped));
    assert_eq!(id(&first), id(&verify(&mut preparation, &original, 1.0)));
    assert_eq!(preparation.inspect().stats.entries, 4);
    let mut items = original.scene().items().to_vec();
    let Item::Text { font, glyphs, .. } = &mut items[0] else {
        panic!("text");
    };
    glyphs[0].id = font.index('Q');
    let different = Frame::new(
        Scene::new(original.extent(), 0, items).unwrap(),
        Visual::new(original.extent()),
    )
    .unwrap();
    assert_ne!(id(&first), id(&verify(&mut preparation, &different, 1.0)));
}
#[test]
fn leases() {
    let mut preparation = Preparation::default();
    preparation
        .command(Command::Configure {
            policy: Policy::bounded(1, 1_048_576),
        })
        .unwrap();
    let first = verify(&mut preparation, &frame(4, 0, None), 1.0);
    let expected = Cpu.submit(&first).unwrap();
    verify(&mut preparation, &frame(8, 0, None), 1.0);
    assert_eq!(preparation.inspect().stats.evicted, 1);
    preparation.command(Command::Clear).unwrap();
    assert_eq!(preparation.inspect().stats.bytes, 0);
    let rebuilt = verify(&mut preparation, &frame(4, 0, None), 1.0);
    assert_ne!(id(&first), id(&rebuilt));
    preparation.command(Command::Destroy).unwrap();
    assert!(preparation.inspect().closed);
    let uncached = verify(&mut preparation, &frame(4, 0, None), 1.0);
    assert_ne!(id(&rebuilt), id(&uncached));
    assert_eq!(preparation.inspect().stats.entries, 0);
    drop(preparation);
    assert_eq!(Cpu.submit(&first).unwrap(), expected);
}
#[test]
fn policy() {
    let mut preparation = Preparation::default();
    let frame = frame(4, 0, None);
    let first = verify(&mut preparation, &frame, 1.0);
    let bytes = preparation.inspect().stats.bytes;
    preparation
        .command(Command::Configure {
            policy: Policy::bounded(64, bytes - 1),
        })
        .unwrap();
    assert_eq!(preparation.inspect().stats.entries, 0);
    let omitted = verify(&mut preparation, &frame, 1.0);
    assert_ne!(id(&first), id(&omitted));
    assert_eq!(preparation.inspect().stats.omitted, 1);
    preparation
        .command(Command::Configure {
            policy: Policy::bounded(64, bytes),
        })
        .unwrap();
    let cached = verify(&mut preparation, &frame, 1.0);
    let mut policy = preparation.inspect().active;
    policy.write = false;
    preparation.command(Command::Configure { policy }).unwrap();
    assert_eq!(id(&cached), id(&verify(&mut preparation, &frame, 1.0)));
    policy.read = false;
    preparation.command(Command::Configure { policy }).unwrap();
    assert_ne!(id(&cached), id(&verify(&mut preparation, &frame, 1.0)));
}
#[test]
fn queue() {
    let frame = frame(4, 0, None);
    let mut queue = Queue::new(Cpu);
    let target = queue.target(frame.extent()).unwrap();
    let first = queue.submit(target, &frame).unwrap();
    let second = queue.submit(target, &frame).unwrap();
    assert_eq!(queue.preparation().inspect().stats.hits, 1);
    queue.preparation().command(Command::Destroy).unwrap();
    queue.poll().unwrap();
    assert_eq!(queue.read(first).unwrap(), queue.read(second).unwrap());
    queue.release(first).unwrap();
    queue.release(second).unwrap();
    queue.close(target).unwrap();
}

#[test]
fn budgets() {
    use graphics::shared::{Ink, Path, Record, Segment, Transform};
    use std::sync::Arc;
    let segments = (0..40)
        .flat_map(|_| {
            [
                Segment::Move([0.into(), 0.into()]),
                Segment::Line([512.into(), 0.into()]),
                Segment::Line([512.into(), 512.into()]),
                Segment::Line([0.into(), 512.into()]),
                Segment::Close,
            ]
        })
        .collect();
    let item = Item::Draw {
        origin: [0; 2],
        clip: None,
        record: Arc::new(
            Record::new(Arc::new(Ink::Path {
                path: Path::new(segments).unwrap(),
                transform: Transform::default(),
                color: 0,
            }))
            .unwrap(),
        ),
    };
    let extent = Extent {
        width: 512,
        height: 512,
    };
    let frame = |count| {
        Frame::new(
            Scene::new(extent, 0, vec![item.clone(); count]).unwrap(),
            Visual::new(extent),
        )
        .unwrap()
    };
    let mut preparation = Preparation::default();
    let output = Output::new(extent, 1.0).unwrap();
    preparation.prepare(&frame(1), output).unwrap();
    assert!(
        preparation
            .prepare(&frame(4), output)
            .unwrap_err()
            .to_string()
            .contains("geometry work budget")
    );
    assert_eq!(preparation.inspect().stats.hits, 3);
}

#[path = "curves/placement.rs"]
mod local;

#[path = "curves/visibility.rs"]
mod visibility;

#[path = "curves/culling.rs"]
mod culling;

#[path = "curves/pruning.rs"]
mod pruning;
