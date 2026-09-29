use graphics::metal::{Metal, Work};
use graphics::shared::{
    Backend, Cpu, Extent, Frame, Ink, Item, Raster, Record, Rect, Scene, State, Visual,
};
use shared::cache::{Command, Policy};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
fn raster(color: [u8; 4]) -> Raster {
    Raster::rgba(
        Extent {
            width: 8,
            height: 8,
        },
        &color.repeat(64),
    )
    .unwrap()
}
fn frame(raster: &Raster) -> Frame {
    let extent = Extent {
        width: 32,
        height: 16,
    };
    let record = Arc::new(
        Record::new(Arc::new(Ink::Image {
            raster: raster.clone(),
            rect: Rect {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
        }))
        .unwrap(),
    );
    Frame::new(
        Scene::new(
            extent,
            0,
            vec![
                Item::Draw {
                    origin: [0, 0],
                    clip: None,
                    record: record.clone(),
                },
                Item::Draw {
                    origin: [16, 0],
                    clip: None,
                    record,
                },
            ],
        )
        .unwrap(),
        Visual::new(extent),
    )
    .unwrap()
}
fn verify(metal: &mut Metal, mut work: Work, frame: &Frame) {
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
    assert_eq!(metal.read(&work).unwrap(), Cpu.render(frame).unwrap());
}
#[test]
fn leases() {
    let mut metal = Metal::new().unwrap();
    let red = frame(&raster([255, 0, 0, 255]));
    let work = metal.render(&red).unwrap();
    assert_eq!(
        metal.inspect(graphics::metal::Resource::Textures).uploads,
        1
    );
    assert_eq!(
        metal.inspect(graphics::metal::Resource::Textures).requests,
        2
    );
    assert_eq!(
        metal.inspect(graphics::metal::Resource::Textures).bytes,
        256
    );
    assert_eq!(
        metal
            .resources(graphics::metal::Resource::Textures)
            .stats
            .entries,
        1
    );
    metal
        .cache(graphics::metal::Resource::Textures, Command::Clear)
        .unwrap();
    assert_eq!(
        metal
            .resources(graphics::metal::Resource::Textures)
            .stats
            .bytes,
        0
    );
    verify(&mut metal, work, &red);
    metal
        .cache(
            graphics::metal::Resource::Textures,
            Command::Configure {
                policy: Policy::bounded(1, 1_048_576),
            },
        )
        .unwrap();
    let pending = metal.render(&red).unwrap();
    let blue = frame(&raster([0, 0, 255, 255]));
    let other = metal.render(&blue).unwrap();
    assert_eq!(
        metal
            .resources(graphics::metal::Resource::Textures)
            .stats
            .entries,
        1
    );
    assert_eq!(
        metal
            .resources(graphics::metal::Resource::Textures)
            .stats
            .evicted,
        1
    );
    metal
        .cache(graphics::metal::Resource::Textures, Command::Destroy)
        .unwrap();
    assert!(metal.resources(graphics::metal::Resource::Textures).closed);
    verify(&mut metal, pending, &red);
    verify(&mut metal, other, &blue);
    let before = metal.inspect(graphics::metal::Resource::Textures);
    let uncached = metal.render(&red).unwrap();
    assert_eq!(
        metal.inspect(graphics::metal::Resource::Textures).uploads - before.uploads,
        2
    );
    verify(&mut metal, uncached, &red);
    assert!(
        metal
            .cache(graphics::metal::Resource::Textures, Command::Clear)
            .is_err()
    );
    let mut rebuilt = Metal::new().unwrap();
    let work = rebuilt.render(&red).unwrap();
    assert_eq!(
        rebuilt.inspect(graphics::metal::Resource::Textures).uploads,
        1
    );
    verify(&mut rebuilt, work, &red);
}
#[test]
fn policy() {
    let mut metal = Metal::new().unwrap();
    let source = raster([0, 255, 0, 255]);
    let first = frame(&source);
    let work = metal.render(&first).unwrap();
    verify(&mut metal, work, &first);
    let allocated = metal
        .resources(graphics::metal::Resource::Textures)
        .stats
        .bytes;
    assert!(allocated >= 256);
    metal
        .cache(
            graphics::metal::Resource::Textures,
            Command::Configure {
                policy: Policy::bounded(64, allocated - 1),
            },
        )
        .unwrap();
    assert_eq!(
        metal
            .resources(graphics::metal::Resource::Textures)
            .stats
            .entries,
        0
    );
    let work = metal.render(&first).unwrap();
    assert_eq!(
        metal.inspect(graphics::metal::Resource::Textures).uploads,
        3
    );
    verify(&mut metal, work, &first);
    metal
        .cache(
            graphics::metal::Resource::Textures,
            Command::Configure {
                policy: Policy::bounded(64, allocated),
            },
        )
        .unwrap();
    let work = metal.render(&first).unwrap();
    verify(&mut metal, work, &first);
    let duplicate = frame(&raster([0, 255, 0, 255]));
    let work = metal.render(&duplicate).unwrap();
    assert_eq!(
        metal.inspect(graphics::metal::Resource::Textures).uploads,
        5
    );
    verify(&mut metal, work, &duplicate);
}
