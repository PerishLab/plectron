use graphics::d3d12::{D3d12, Work};
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
fn verify(d3d12: &mut D3d12, mut work: Work, frame: &Frame) {
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
    assert_eq!(d3d12.read(&work).unwrap(), Cpu.render(frame).unwrap());
}
#[test]
fn leases() {
    let mut d3d12 = D3d12::new().unwrap();
    let red = frame(&raster([255, 0, 0, 255]));
    let work = d3d12.render(&red).unwrap();
    assert_eq!(
        d3d12.inspect(graphics::d3d12::Resource::Textures).uploads,
        1
    );
    assert_eq!(
        d3d12.inspect(graphics::d3d12::Resource::Textures).requests,
        2
    );
    assert_eq!(
        d3d12.inspect(graphics::d3d12::Resource::Textures).bytes,
        256
    );
    assert_eq!(
        d3d12
            .resources(graphics::d3d12::Resource::Textures)
            .stats
            .entries,
        1
    );
    d3d12
        .cache(graphics::d3d12::Resource::Textures, Command::Clear)
        .unwrap();
    assert_eq!(
        d3d12
            .resources(graphics::d3d12::Resource::Textures)
            .stats
            .bytes,
        0
    );
    verify(&mut d3d12, work, &red);
    d3d12
        .cache(
            graphics::d3d12::Resource::Textures,
            Command::Configure {
                policy: Policy::bounded(1, 1_048_576),
            },
        )
        .unwrap();
    let pending = d3d12.render(&red).unwrap();
    let blue = frame(&raster([0, 0, 255, 255]));
    let other = d3d12.render(&blue).unwrap();
    assert_eq!(
        d3d12
            .resources(graphics::d3d12::Resource::Textures)
            .stats
            .entries,
        1
    );
    assert_eq!(
        d3d12
            .resources(graphics::d3d12::Resource::Textures)
            .stats
            .evicted,
        1
    );
    d3d12
        .cache(graphics::d3d12::Resource::Textures, Command::Destroy)
        .unwrap();
    assert!(d3d12.resources(graphics::d3d12::Resource::Textures).closed);
    verify(&mut d3d12, pending, &red);
    verify(&mut d3d12, other, &blue);
    let before = d3d12.inspect(graphics::d3d12::Resource::Textures);
    let uncached = d3d12.render(&red).unwrap();
    assert_eq!(
        d3d12.inspect(graphics::d3d12::Resource::Textures).uploads - before.uploads,
        2
    );
    verify(&mut d3d12, uncached, &red);
    assert!(
        d3d12
            .cache(graphics::d3d12::Resource::Textures, Command::Clear)
            .is_err()
    );
    let mut rebuilt = D3d12::new().unwrap();
    let work = rebuilt.render(&red).unwrap();
    assert_eq!(
        rebuilt.inspect(graphics::d3d12::Resource::Textures).uploads,
        1
    );
    verify(&mut rebuilt, work, &red);
}
#[test]
fn policy() {
    let mut d3d12 = D3d12::new().unwrap();
    let source = raster([0, 255, 0, 255]);
    let first = frame(&source);
    let work = d3d12.render(&first).unwrap();
    verify(&mut d3d12, work, &first);
    let allocated = d3d12
        .resources(graphics::d3d12::Resource::Textures)
        .stats
        .bytes;
    assert!(allocated >= 256);
    d3d12
        .cache(
            graphics::d3d12::Resource::Textures,
            Command::Configure {
                policy: Policy::bounded(64, allocated - 1),
            },
        )
        .unwrap();
    assert_eq!(
        d3d12
            .resources(graphics::d3d12::Resource::Textures)
            .stats
            .entries,
        0
    );
    let work = d3d12.render(&first).unwrap();
    assert_eq!(
        d3d12.inspect(graphics::d3d12::Resource::Textures).uploads,
        3
    );
    verify(&mut d3d12, work, &first);
    d3d12
        .cache(
            graphics::d3d12::Resource::Textures,
            Command::Configure {
                policy: Policy::bounded(64, allocated),
            },
        )
        .unwrap();
    let work = d3d12.render(&first).unwrap();
    verify(&mut d3d12, work, &first);
    let duplicate = frame(&raster([0, 255, 0, 255]));
    let work = d3d12.render(&duplicate).unwrap();
    assert_eq!(
        d3d12.inspect(graphics::d3d12::Resource::Textures).uploads,
        5
    );
    verify(&mut d3d12, work, &duplicate);
}
