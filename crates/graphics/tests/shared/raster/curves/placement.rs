use graphics::shared::{
    Backend, Cpu, Evaluation, Extent, Frame, Ink, Item, Output, Path, Preparation, Primitive,
    Rasterization, Record, Rect, Scalar, Scene, Segment, Transform, Visual,
};
use shared::cache::Command;
use std::sync::Arc;

fn frame(offset: [f64; 2], clip: Option<Rect>, color: u32) -> Frame {
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([24.into(), 0.into()], [24.into(), 24.into()]),
        Segment::Quad([0.into(), 24.into()], [0.into(), 0.into()]),
        Segment::Close,
    ])
    .unwrap();
    let item = Item::Draw {
        origin: [0; 2],
        clip: clip.map(Into::into),
        record: Arc::new(
            Record::new(Arc::new(Ink::Path {
                path,
                transform: Transform::default().translate(offset.map(Scalar::new)),
                color,
            }))
            .unwrap(),
        ),
    };
    let extent = Extent {
        width: 128,
        height: 96,
    };
    Frame::new(
        Scene::new(extent, 0, vec![item]).unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}
fn prepare(preparation: &mut Preparation, frame: &Frame, scale: f64) -> graphics::shared::Pass {
    preparation
        .prepare(frame, Output::new(frame.extent(), scale).unwrap())
        .unwrap()
}
fn placement(pass: &graphics::shared::Pass) -> &graphics::shared::preparation::Placement {
    let Primitive::Coverage(placement, _) = &pass.layer().primitives[0] else {
        panic!("coverage")
    };
    placement
}
fn local() -> Preparation {
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    preparation.rasterization(Rasterization::Local);
    preparation
}
#[test]
fn translation() {
    let mut preparation = local();
    let first = prepare(&mut preparation, &frame([4.375, 5.25], None, 0), 1.0);
    let next = prepare(&mut preparation, &frame([43.375, 19.25], None, 0), 1.0);
    assert_eq!(
        placement(&first).coverage.id(),
        placement(&next).coverage.id()
    );
    assert_eq!(placement(&first).offset, [4, 5]);
    assert_eq!(placement(&next).offset, [43, 19]);
    assert_eq!(preparation.rasterizer().report().builds, 1);
    let a = Cpu.submit(&first).unwrap();
    let b = Cpu.submit(&next).unwrap();
    for y in 0..25 {
        for x in 0..25 {
            assert_eq!(
                a.pixels[(y + 5) * 128 + x + 4],
                b.pixels[(y + 19) * 128 + x + 43]
            );
        }
    }
    preparation.command(Command::Destroy).unwrap();
    preparation.rasterizer().command(Command::Destroy).unwrap();
    drop(preparation);
    assert_eq!(Cpu.submit(&first).unwrap(), a);
}
#[test]
fn invalidation() {
    let mut preparation = local();
    let base = frame([4.375, 5.25], None, 0);
    let first = prepare(&mut preparation, &base, 1.0);
    let id = placement(&first).coverage.id();
    let colored = prepare(&mut preparation, &frame([4.375, 5.25], None, 0xff0000), 1.0);
    assert_eq!(id, placement(&colored).coverage.id());
    let outside = Some(Rect {
        x: 2,
        y: 2,
        width: 80,
        height: 70,
    });
    let same = prepare(&mut preparation, &frame([4.375, 5.25], outside, 0), 1.0);
    assert_eq!(id, placement(&same).coverage.id());
    let phase = prepare(&mut preparation, &frame([4.5, 5.25], None, 0), 1.0);
    assert_ne!(id, placement(&phase).coverage.id());
    let scale = prepare(&mut preparation, &base, 1.25);
    assert_ne!(id, placement(&scale).coverage.id());
    let inside = Some(Rect {
        x: 8,
        y: 9,
        width: 13,
        height: 12,
    });
    let clipped = prepare(&mut preparation, &frame([4.375, 5.25], inside, 0), 1.25);
    let again = prepare(&mut local(), &frame([4.375, 5.25], inside, 0), 1.25);
    assert_eq!(Cpu.submit(&clipped).unwrap(), Cpu.submit(&again).unwrap());
    assert_ne!(id, placement(&clipped).coverage.id());
    assert_eq!(preparation.rasterizer().report().builds, 4);
}
#[test]
fn oracle() {
    for evaluation in [Evaluation::Linear, Evaluation::Quadratic] {
        let mut preparation = local();
        preparation.evaluation(evaluation);
        let base = frame([0.375, 0.25], None, 0x224466);
        let mut direct = Preparation::default();
        direct.evaluation(evaluation);
        let expected = Cpu.submit(&prepare(&mut direct, &base, 1.0)).unwrap();
        for offset in [[0.375, 0.25], [42.375, 31.25]] {
            let shifted = frame(offset, None, 0x224466);
            let pass = prepare(&mut preparation, &shifted, 1.0);
            let actual = Cpu.submit(&pass).unwrap();
            translated(&expected, &actual, offset.map(|v| v as usize));
        }
    }
}
#[test]
fn clipping() {
    let mut preparation = local();
    let mut direct = Preparation::default();
    direct.evaluation(Evaluation::Quadratic);
    for offset in [[-8.0, -7.0], [120.0, 90.0], [130.0, 4.0], [-40.0, 0.0]] {
        let frame = frame(offset, None, 0);
        let local = prepare(&mut preparation, &frame, 1.0);
        let global = prepare(&mut direct, &frame, 1.0);
        assert_eq!(Cpu.submit(&local).unwrap(), Cpu.submit(&global).unwrap());
    }
}

fn translated(
    expected: &graphics::shared::Image,
    actual: &graphics::shared::Image,
    offset: [usize; 2],
) {
    for y in 0..25 {
        for x in 0..25 {
            let at = (y + offset[1]) * 128 + x + offset[0];
            assert_eq!(actual.pixels[at], expected.pixels[y * 128 + x]);
        }
    }
}
