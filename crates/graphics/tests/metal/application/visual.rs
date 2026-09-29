#![cfg(target_os = "macos")]
use graphics::metal::Metal;
use graphics::shared::{
    Backend, Cpu, Extent, Font, Frame, Glyph, Item, Queue, Rect, Scene, Visual,
};
use std::time::Duration;

#[test]
fn scrolling() {
    let extent = Extent {
        width: 64,
        height: 96,
    };
    let mut items: Vec<_> = [0x112233, 0x445566, 0x778899]
        .into_iter()
        .enumerate()
        .map(|(index, color)| Item::Fill {
            rect: Rect {
                x: 0,
                y: index * 32,
                width: 64,
                height: 32,
            },
            color: Some(color),
        })
        .collect();
    items.push(Item::Text {
        font: Font::default(),
        clip: Some(
            Rect {
                x: 0,
                y: 0,
                width: 64,
                height: 96,
            }
            .into(),
        ),
        glyphs: vec![Glyph {
            id: 128,
            x: (4).into(),
            y: (34).into(),
        }],
        color: 0xffffff,
    });
    let scene = Scene::new(extent, 1, items).unwrap();
    let viewport = Extent {
        width: 64,
        height: 32,
    };
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(viewport).unwrap();
    for offset in [0, 17, 32, 48, 64] {
        let frame = Frame::new(
            scene.clone(),
            Visual {
                extent: viewport,
                offsets: std::collections::BTreeMap::from([(0, offset)]),
                revision: offset as u64,
            },
        )
        .unwrap();
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        assert_eq!(queue.read(ticket).unwrap(), Cpu.render(&frame).unwrap());
        assert_eq!(frame.scene().id(), scene.id());
        queue.release(ticket).unwrap();
    }
    assert!(
        Frame::new(
            scene,
            Visual {
                extent: viewport,
                offsets: std::collections::BTreeMap::from([(0, 65)]),
                revision: 0
            }
        )
        .is_err()
    );
    queue.close(target).unwrap();
}
