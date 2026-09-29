#![cfg(target_os = "macos")]
use graphics::metal::Metal;
use graphics::shared::{
    Backend, Cpu, Extent, Font, Frame, Glyph, Ink, Item, Queue, Record, Rect, Scene, Space, Visual,
};
use std::sync::Arc;
use std::time::Duration;

fn clip() -> Rect {
    Rect {
        x: 4,
        y: 2,
        width: 40,
        height: 30,
    }
}
fn frame(item: Item, translation: [i64; 2], opacity: u8) -> Frame {
    let extent = Extent {
        width: 64,
        height: 40,
    };
    let items = vec![Item::Group {
        space: Space {
            id: 1,
            translation,
            clip: Some(clip().into()),
            scroll: Some(16),
        },
        opacity,
        items: vec![item].into(),
    }];
    let mut visual = Visual::new(extent);
    visual.offsets.insert(1, 4);
    Frame::new(Scene::new(extent, 1, items).unwrap(), visual).unwrap()
}

#[test]
fn retained() {
    let glyphs = vec![
        Glyph {
            id: Font::default().index('A'),
            x: (0).into(),
            y: (0).into(),
        },
        Glyph {
            id: 128,
            x: (16).into(),
            y: (0).into(),
        },
    ];
    let record = Arc::new(
        Record::new(Arc::new(Ink::Text {
            font: Font::default(),
            glyphs: glyphs.clone().into(),
            color: 0x224488,
        }))
        .unwrap(),
    );
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue
        .target(Extent {
            width: 64,
            height: 40,
        })
        .unwrap();
    let mut frames = Vec::new();
    for (origin, translation, opacity) in [
        ([6, 8], [0, 0], 255),
        ([10, 12], [-4, 2], 128),
        ([24, 8], [4, -2], 64),
    ] {
        let actual = frame(
            Item::Draw {
                origin,
                clip: Some(clip().into()),
                record: record.clone(),
            },
            translation,
            opacity,
        );
        let expected = frame(
            Item::Text {
                font: Font::default(),
                glyphs: glyphs
                    .iter()
                    .map(|glyph| Glyph {
                        id: glyph.id,
                        x: glyph.x + (origin[0] as i64).into(),
                        y: glyph.y + (origin[1] as i64).into(),
                    })
                    .collect(),
                color: 0x224488,
                clip: Some(clip().into()),
            },
            translation,
            opacity,
        );
        let pixels = Cpu.render(&expected).unwrap();
        assert_eq!(Cpu.render(&actual).unwrap(), pixels);
        frames.push((actual, pixels));
    }
    drop(record);
    for (frame, expected) in frames.into_iter().rev() {
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        let actual = queue.read(ticket).unwrap();
        for (actual, expected) in actual.pixels.iter().zip(&expected.pixels) {
            for shift in [0, 8, 16] {
                assert!(((actual >> shift) & 255).abs_diff((expected >> shift) & 255) <= 1);
            }
        }
        queue.release(ticket).unwrap();
    }
    queue.close(target).unwrap();
}
