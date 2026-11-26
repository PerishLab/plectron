use crate::d3d12::{D3d12, Resource, Work};
use crate::shared::{
    Backend, Cpu, Extent, Font, Frame, Glyph, Item, Output, Pass, Preparation, Scene, State, Visual,
};
use shared::cache::{Command, Policy};
use std::time::{Duration, Instant};

fn frame(position: i64, color: u32) -> Frame {
    let font = Font {
        face: fixtures::face("lato", crate::shared::Weight::Normal),
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
                clip: None,
            }],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}
fn pass(preparation: &mut Preparation, position: i64, color: u32) -> Pass {
    let frame = frame(position, color);
    preparation
        .prepare(&frame, Output::new(frame.extent(), 1.0).unwrap())
        .unwrap()
}
fn verify(d3d12: &mut D3d12, mut work: Work, expected: crate::shared::Image) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        match d3d12.poll(&mut work).unwrap() {
            State::Complete => break,
            State::Failed(fault) => panic!("{fault}"),
            State::Pending => (),
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(d3d12.read(&work).unwrap(), expected);
}
#[test]
fn leases() {
    let mut preparation = Preparation::default();
    let mut d3d12 = D3d12::new().unwrap();
    d3d12
        .cache(
            Resource::Paths,
            Command::Configure {
                policy: Policy::bounded(1, 1_048_576),
            },
        )
        .unwrap();
    let original = pass(&mut preparation, 4, 0);
    let crate::shared::Primitive::Path(coverage, _) = &original.layer().primitives[0] else {
        panic!("path");
    };
    let weak = std::sync::Arc::downgrade(coverage);
    let first = d3d12.submit(&original).unwrap();
    let firstpixels = Cpu.submit(&original).unwrap();
    let colored = pass(&mut preparation, 4, 0xff0000);
    let second = d3d12.submit(&colored).unwrap();
    assert_eq!(d3d12.inspect(Resource::Paths).requests, 2);
    assert_eq!(d3d12.inspect(Resource::Paths).uploads, 1);
    let moved = pass(&mut preparation, 8, 0x004400);
    let third = d3d12.submit(&moved).unwrap();
    assert_eq!(d3d12.resources(Resource::Paths).stats.evicted, 1);
    let secondpixels = Cpu.submit(&colored).unwrap();
    let thirdpixels = Cpu.submit(&moved).unwrap();
    drop(original);
    drop(colored);
    drop(moved);
    drop(preparation);
    assert!(weak.upgrade().is_none());
    d3d12.cache(Resource::Paths, Command::Destroy).unwrap();
    assert!(d3d12.resources(Resource::Paths).closed);
    assert!(!d3d12.resources(Resource::Textures).closed);
    verify(&mut d3d12, first, firstpixels);
    verify(&mut d3d12, second, secondpixels);
    verify(&mut d3d12, third, thirdpixels);
    let original = frame(4, 0);
    let work = d3d12.render(&original).unwrap();
    verify(&mut d3d12, work, Cpu.render(&original).unwrap());
    assert_eq!(d3d12.inspect(Resource::Paths).uploads, 3);
}
#[test]
fn devices() {
    let mut preparation = Preparation::default();
    let pass = pass(&mut preparation, 4, 0);
    let mut first = D3d12::new().unwrap();
    let work = first.submit(&pass).unwrap();
    let bytes = first.resources(Resource::Paths).stats.bytes;
    assert!(bytes > 0);
    first
        .cache(
            Resource::Paths,
            Command::Configure {
                policy: Policy::bounded(64, bytes - 1),
            },
        )
        .unwrap();
    assert_eq!(first.resources(Resource::Paths).stats.bytes, 0);
    let mut second = D3d12::new().unwrap();
    assert!(second.read(&work).is_err());
    let other = second.submit(&pass).unwrap();
    assert_eq!(second.inspect(Resource::Paths).uploads, 1);
    second.cache(Resource::Paths, Command::Clear).unwrap();
    verify(&mut second, other, Cpu.submit(&pass).unwrap());
    verify(&mut first, work, Cpu.submit(&pass).unwrap());
}

#[test]
fn vertices() {
    let mut d3d12 = D3d12::new().unwrap();
    for count in [34, 35, 256] {
        let original = frame(4, 0x223344);
        let fills = |count: usize| {
            (0..count).map(|index| Item::Fill {
                rect: crate::shared::Rect {
                    x: index % 80,
                    y: index / 80,
                    width: 1,
                    height: 1,
                },
                color: Some(0x226622),
            })
        };
        let items = fills(count)
            .chain(original.scene().items().iter().cloned())
            .chain(fills(34))
            .collect();
        let frame = Frame::new(
            Scene::new(original.extent(), 0, items).unwrap(),
            Visual::new(original.extent()),
        )
        .unwrap();
        let work = d3d12.render(&frame).unwrap();
        verify(&mut d3d12, work, Cpu.render(&frame).unwrap());
    }
}
