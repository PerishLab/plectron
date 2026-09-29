#![cfg(target_os = "macos")]
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Frame, Item, Queue, Rect, Scene, Space, Visual};
use std::time::Duration;

fn group(id: u64, opacity: u8, items: Vec<Item>) -> Item {
    Item::Group {
        space: Space {
            id,
            translation: [0, 0],
            clip: None,
            scroll: None,
        },
        opacity,
        items: items.into(),
    }
}
fn fill(x: usize, y: usize, color: u32) -> Item {
    Item::Fill {
        rect: Rect {
            x,
            y,
            width: 24,
            height: 24,
        },
        color: Some(color),
    }
}
#[test]
fn compositing() {
    let extent = Extent {
        width: 32,
        height: 32,
    };
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    for opacity in [0, 1, 64, 127, 128, 254, 255] {
        let scene = Scene::new(
            extent,
            1,
            vec![
                group(1, opacity, vec![fill(0, 0, 0xff0000), fill(8, 8, 0x0000ff)]),
                group(2, 128, vec![group(3, 128, vec![fill(16, 0, 0x228844)])]),
            ],
        )
        .unwrap();
        let frame = Frame::new(scene, Visual::new(extent)).unwrap();
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        let gpu = queue.read(ticket).unwrap();
        let cpu = Cpu.render(&frame).unwrap();
        for (actual, expected) in gpu.pixels.iter().zip(&cpu.pixels) {
            for shift in [0, 8, 16] {
                assert!(
                    ((actual >> shift) & 255).abs_diff((expected >> shift) & 255) <= 2,
                    "{actual:06x} != {expected:06x} at opacity {opacity}"
                );
            }
        }
        queue.release(ticket).unwrap();
    }
    queue.close(target).unwrap();
}
#[test]
fn example() {
    for method in [
        graphics::shared::Rasterization::Strips,
        graphics::shared::Rasterization::Local,
    ] {
        application(method);
    }
}
fn application(method: graphics::shared::Rasterization) {
    let extent = Extent {
        width: 360,
        height: 240,
    };
    let mut page = webengine::Page::new("app://spaces/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("spaces")).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    queue.preparation().rasterization(method);
    let target = queue.target(extent).unwrap();
    for delta in [0, 24, 32, -16] {
        page.scroll(delta).unwrap();
        let frame = page.frame().unwrap();
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        let gpu = queue.read(ticket).unwrap();
        let direct = Cpu.render(&frame).unwrap();
        let mut preparation = graphics::shared::Preparation::default();
        preparation.rasterization(method);
        let output = graphics::shared::Output::new(extent, 1.0).unwrap();
        let cpu = Cpu
            .submit(&preparation.prepare(&frame, output).unwrap())
            .unwrap();
        let coordinate = difference(&cpu, &direct);
        let changed = cpu
            .pixels
            .iter()
            .zip(&direct.pixels)
            .filter(|(a, b)| a != b)
            .count();
        let maximum = difference(&cpu, &gpu);
        eprintln!(
            "spaces {method:?} scroll {delta}: coordinates {coordinate}, changed {changed}, Metal {maximum}"
        );
        if method == graphics::shared::Rasterization::Strips {
            assert_eq!(cpu, direct);
        }
        assert!(maximum <= 1);
        page.acknowledge(frame.id()).unwrap();
        page.wheel(40, 130, 8).unwrap();
        queue.release(ticket).unwrap();
    }
    page.close();
    queue.close(target).unwrap();
}

fn difference(a: &graphics::shared::Image, b: &graphics::shared::Image) -> u32 {
    assert_eq!(a.extent, b.extent);
    a.pixels
        .iter()
        .zip(&b.pixels)
        .flat_map(|(a, b)| {
            [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255))
        })
        .max()
        .unwrap()
}
