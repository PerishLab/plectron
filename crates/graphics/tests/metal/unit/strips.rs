use super::{batches, regions};
use crate::metal::{Metal, Resource};
use crate::shared::{Backend, Cpu, Evaluation, Output, Preparation, Rasterization};
use shared::cache::{Command, Policy};

#[test]
fn oracle() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    for evaluation in [Evaluation::Linear, Evaluation::Quadratic] {
        preparation.evaluation(evaluation);
        for scale in [1.0, 1.25, 2.0] {
            let frame = regions::frame(0x224488);
            let output = Output::new(frame.extent(), scale).unwrap();
            preparation.rasterization(Rasterization::Direct);
            let direct = preparation.prepare(&frame, output).unwrap();
            let expected = Cpu.submit(&direct).unwrap();
            preparation.rasterization(Rasterization::Strips);
            let pass = preparation.prepare(&frame, output).unwrap();
            assert_eq!(Cpu.submit(&pass).unwrap(), expected);
            let work = metal.submit(&pass).unwrap();
            regions::read(&mut metal, &work, &expected);
        }
    }
}
#[test]
fn retention() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    preparation.rasterization(Rasterization::Strips);
    let frame = batches::frame(vec![batches::item([7, 5], 0x112233)]);
    let output = Output::new(frame.extent(), 1.25).unwrap();
    let pass = preparation.prepare(&frame, output).unwrap();
    let first = metal.submit(&pass).unwrap();
    assert_eq!(preparation.rasterizer().report().builds, 1);
    let frame = batches::frame(vec![batches::item([7, 5], 0xff0000)]);
    let colored = preparation.prepare(&frame, output).unwrap();
    let second = metal.submit(&colored).unwrap();
    assert_eq!(preparation.rasterizer().report().builds, 1);
    assert_eq!(metal.inspect(Resource::Coverages).uploads, 1);
    preparation.rasterizer().command(Command::Destroy).unwrap();
    preparation.command(Command::Destroy).unwrap();
    metal.cache(Resource::Coverages, Command::Destroy).unwrap();
    drop(preparation);
    batches::read(&mut metal, first, &Cpu.submit(&pass).unwrap());
    batches::read(&mut metal, second, &Cpu.submit(&colored).unwrap());
    let third = metal.submit(&pass).unwrap();
    batches::read(&mut metal, third, &Cpu.submit(&pass).unwrap());
    assert_eq!(metal.inspect(Resource::Coverages).uploads, 2);
}
#[test]
fn eviction() {
    let mut metal = Metal::new().unwrap();
    metal
        .cache(
            Resource::Coverages,
            Command::Configure {
                policy: Policy::bounded(1, 33_554_432),
            },
        )
        .unwrap();
    let mut preparation = Preparation::default();
    preparation.rasterization(Rasterization::Strips);
    preparation
        .rasterizer()
        .command(Command::Configure {
            policy: Policy::bounded(1, 33_554_432),
        })
        .unwrap();
    let frame = batches::frame(
        (0..17)
            .map(|i| batches::item([i * 16, 7], i as u32 * 19273))
            .collect(),
    );
    let output = Output::new(frame.extent(), 1.0).unwrap();
    let pass = preparation.prepare(&frame, output).unwrap();
    let first = metal.submit(&pass).unwrap();
    let second = metal.submit(&pass).unwrap();
    assert_eq!(preparation.rasterizer().inspect().stats.evicted, 16);
    assert_eq!(metal.resources(Resource::Coverages).stats.evicted, 33);
    assert_eq!(metal.inspect(Resource::Paths).uploads, 0);
    let expected = Cpu.submit(&pass).unwrap();
    batches::read(&mut metal, first, &expected);
    batches::read(&mut metal, second, &expected);
    let mut other = Metal::new().unwrap();
    let work = other.submit(&pass).unwrap();
    assert_eq!(other.inspect(Resource::Coverages).uploads, 17);
    batches::read(&mut other, work, &expected);
}

#[test]
fn placement() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    preparation.evaluation(Evaluation::Quadratic);
    preparation.rasterization(Rasterization::Local);
    let mut held = Vec::new();
    for step in 0..4 {
        let frame = batches::frame(vec![
            batches::item([step * 9 + 7, 5], 0x112233),
            batches::item([step * 9 + 7, 11], 0x883355),
            batches::item([step * 9 + 80, 24], 0x553388),
        ]);
        let output = Output::new(frame.extent(), 1.0).unwrap();
        let pass = preparation.prepare(&frame, output).unwrap();
        let work = metal.submit(&pass).unwrap();
        held.push((work, Cpu.submit(&pass).unwrap()));
    }
    assert_eq!(preparation.rasterizer().report().builds, 1);
    assert_eq!(metal.inspect(Resource::Coverages).uploads, 1);
    preparation.command(Command::Destroy).unwrap();
    preparation.rasterizer().command(Command::Destroy).unwrap();
    metal.cache(Resource::Coverages, Command::Destroy).unwrap();
    drop(preparation);
    for (work, expected) in held {
        batches::read(&mut metal, work, &expected);
    }
}
#[test]
fn local() {
    let mut metal = Metal::new().unwrap();
    let mut preparation = Preparation::default();
    preparation.rasterization(Rasterization::Local);
    for scale in [1.0, 1.25, 2.0] {
        let frame = regions::frame(0x224488);
        let output = Output::new(frame.extent(), scale).unwrap();
        let pass = preparation.prepare(&frame, output).unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        let work = metal.submit(&pass).unwrap();
        regions::read(&mut metal, &work, &expected);
    }
}
