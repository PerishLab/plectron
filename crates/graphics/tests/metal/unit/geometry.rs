use crate::metal::{Metal, Resource, Work};
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
fn verify(metal: &mut Metal, mut work: Work, expected: crate::shared::Image) {
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
    assert_eq!(metal.read(&work).unwrap(), expected);
}
#[test]
fn leases() {
    let mut preparation = Preparation::default();
    let mut metal = Metal::new().unwrap();
    metal
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
    let first = metal.submit(&original).unwrap();
    let firstpixels = Cpu.submit(&original).unwrap();
    let colored = pass(&mut preparation, 4, 0xff0000);
    let second = metal.submit(&colored).unwrap();
    assert_eq!(metal.inspect(Resource::Paths).requests, 2);
    assert_eq!(metal.inspect(Resource::Paths).uploads, 1);
    let moved = pass(&mut preparation, 8, 0x004400);
    let third = metal.submit(&moved).unwrap();
    assert_eq!(metal.resources(Resource::Paths).stats.evicted, 1);
    let secondpixels = Cpu.submit(&colored).unwrap();
    let thirdpixels = Cpu.submit(&moved).unwrap();
    drop(original);
    drop(colored);
    drop(moved);
    drop(preparation);
    assert!(weak.upgrade().is_none());
    metal.cache(Resource::Paths, Command::Destroy).unwrap();
    assert!(metal.resources(Resource::Paths).closed);
    assert!(!metal.resources(Resource::Textures).closed);
    verify(&mut metal, first, firstpixels);
    verify(&mut metal, second, secondpixels);
    verify(&mut metal, third, thirdpixels);
    let original = frame(4, 0);
    let work = metal.render(&original).unwrap();
    verify(&mut metal, work, Cpu.render(&original).unwrap());
    assert_eq!(metal.inspect(Resource::Paths).uploads, 3);
}
#[test]
fn devices() {
    let mut preparation = Preparation::default();
    let pass = pass(&mut preparation, 4, 0);
    let mut first = Metal::new().unwrap();
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
    let mut second = Metal::new().unwrap();
    assert!(second.read(&work).is_err());
    let other = second.submit(&pass).unwrap();
    assert_eq!(second.inspect(Resource::Paths).uploads, 1);
    second.cache(Resource::Paths, Command::Clear).unwrap();
    verify(&mut second, other, Cpu.submit(&pass).unwrap());
    verify(&mut first, work, Cpu.submit(&pass).unwrap());
}

#[test]
fn vertices() {
    let mut metal = Metal::new().unwrap();
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
        let work = metal.render(&frame).unwrap();
        verify(&mut metal, work, Cpu.render(&frame).unwrap());
    }
}
