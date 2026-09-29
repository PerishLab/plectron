use super::{batches, regions};
use crate::metal::{Metal, Resource};
use crate::shared::{Backend, Cpu, Evaluation, Output, Preparation};
use shared::cache::Command;

#[test]
fn evaluation() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    let frame = batches::frame(
        (0..65)
            .map(|i| batches::item([i % 32 * 16, i / 32 * 16], 0x334455))
            .collect(),
    );
    for scale in [1.0, 1.25, 2.0] {
        let pass = preparation
            .prepare(&frame, Output::new(frame.extent(), scale).unwrap())
            .unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        for limit in [1, 128] {
            metal.batching(limit).unwrap();
            let work = metal.submit(&pass).unwrap();
            batches::read(&mut metal, work, &expected);
        }
    }
}
#[test]
fn layers() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    for scale in [1.0, 1.25, 2.0] {
        let frame = regions::frame(0x884422);
        let pass = preparation
            .prepare(&frame, Output::new(frame.extent(), scale).unwrap())
            .unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        let first = metal.submit(&pass).unwrap();
        metal.cache(Resource::Paths, Command::Clear).unwrap();
        let second = metal.submit(&pass).unwrap();
        regions::read(&mut metal, &first, &expected);
        regions::read(&mut metal, &second, &expected);
    }
}
#[test]
fn identities() {
    let frame = batches::frame(vec![batches::item([7, 5], 0x112233)]);
    let output = Output::new(frame.extent(), 1.0).unwrap();
    let mut preparation = Preparation::default();
    let first = preparation.prepare(&frame, output).unwrap();
    let before = preparation.inspect().stats.hits;
    preparation.evaluation(Evaluation::Quadratic);
    let second = preparation.prepare(&frame, output).unwrap();
    assert_eq!(preparation.inspect().stats.hits, before);
    preparation.evaluation(Evaluation::Linear);
    preparation.prepare(&frame, output).unwrap();
    assert_eq!(preparation.inspect().stats.hits, before + 1);
    let mut metal = Metal::new().unwrap();
    let linear = metal.submit(&first).unwrap();
    let quadratic = metal.submit(&second).unwrap();
    assert_eq!(metal.inspect(Resource::Paths).uploads, 2);
    metal.cache(Resource::Paths, Command::Destroy).unwrap();
    preparation.command(Command::Destroy).unwrap();
    batches::read(&mut metal, linear, &Cpu.submit(&first).unwrap());
    batches::read(&mut metal, quadratic, &Cpu.submit(&second).unwrap());
}

#[test]
fn topology() {
    use crate::shared::{Ink, Item, Path, Record, Scalar, Segment, Transform};
    use std::sync::Arc;
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([70.into(), (-20).into()], [60.into(), 40.into()]),
        Segment::Quad([(-10).into(), 60.into()], [0.into(), 0.into()]),
        Segment::Close,
        Segment::Move([15.into(), 10.into()]),
        Segment::Line([15.into(), 25.into()]),
        Segment::Quad([40.into(), 40.into()], [40.into(), 10.into()]),
        Segment::Close,
    ])
    .unwrap();
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    for reflection in [-1.0, 1.0] {
        let frame = batches::frame(vec![Item::Draw {
            origin: [80, 20],
            clip: None,
            record: Arc::new(
                Record::new(Arc::new(Ink::Path {
                    path: path.clone(),
                    color: 0x336677,
                    transform: Transform([
                        Scalar::new(reflection),
                        Scalar::new(0.137),
                        0.into(),
                        1.into(),
                        Scalar::new(0.213),
                        0.into(),
                    ]),
                }))
                .unwrap(),
            ),
        }]);
        let pass = preparation
            .prepare(&frame, Output::new(frame.extent(), 1.25).unwrap())
            .unwrap();
        let work = metal.submit(&pass).unwrap();
        batches::read(&mut metal, work, &Cpu.submit(&pass).unwrap());
    }
}

#[test]
fn routing() {
    use crate::shared::{Item, Rect};
    let fill = |x| Item::Fill {
        rect: Rect {
            x,
            y: 1,
            width: 3,
            height: 3,
        },
        color: Some(0x224466),
    };
    let frame = batches::frame(vec![
        fill(1),
        batches::item([16, 0], 0xff0000),
        fill(33),
        batches::item([48, 0], 0x00ff00),
    ]);
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    let pass = preparation
        .prepare(&frame, Output::new(frame.extent(), 1.25).unwrap())
        .unwrap();
    let mut metal = Metal::new().unwrap();
    let work = metal.submit(&pass).unwrap();
    assert_eq!(metal.batches().switched, 3);
    batches::read(&mut metal, work, &Cpu.submit(&pass).unwrap());
}
