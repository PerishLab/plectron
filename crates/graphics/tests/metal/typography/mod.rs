#![cfg(target_os = "macos")]
mod clusters;
mod continuation;
mod density;
mod direction;
mod mixing;
mod reshaping;
mod wrapping;
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Queue};
use std::time::Duration;
use webengine::Page;

#[test]
fn example() {
    let extent = Extent {
        width: 720,
        height: 640,
    };
    let mut page = Page::new("app://typography/index.html", extent).unwrap();
    page.load(&mut crate::Source::new("typography")).unwrap();
    let original = page.frame().unwrap();
    let pixels = Cpu.render(&original).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    let mut previous = None;
    for action in ["", "size", "leading", "align"] {
        if !action.is_empty() {
            page.evaluate(&format!(
                "document.getElementById('{action}').dispatchEvent('click', '');"
            ))
            .unwrap();
            page.render().unwrap();
        }
        let frame = if action.is_empty() {
            original.clone()
        } else {
            page.frame().unwrap()
        };
        let expected = Cpu.render(&frame).unwrap();
        if let Some(previous) = &previous {
            assert_ne!(previous, &expected);
        }
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(3)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        queue.release(ticket).unwrap();
        page.acknowledge(frame.id()).unwrap();
        previous = Some(expected);
    }
    page.close();
    let ticket = queue.submit(target, &original).unwrap();
    queue.wait(ticket, Duration::from_secs(3)).unwrap();
    compare(&queue.read(ticket).unwrap(), &pixels);
    queue.release(ticket).unwrap();
    queue.close(target).unwrap();
}

#[test]
fn signed() {
    use graphics::shared::{Font, Frame, Glyph, Ink, Item, Record, Scene, Space, Visual};
    use std::sync::Arc;
    let extent = Extent {
        width: 64,
        height: 64,
    };
    let font = Font {
        size: 23.into(),
        weight: graphics::shared::Weight::Bold,
        ..Font::default()
    };
    let actual = Item::Group {
        space: Space {
            id: 1,
            translation: [12, 14],
            clip: None,
            scroll: None,
        },
        opacity: 255,
        items: vec![Item::Draw {
            origin: [4, 4],
            clip: None,
            record: Arc::new(
                Record::new(Arc::new(Ink::Text {
                    font: font.clone(),
                    color: 0,
                    glyphs: vec![Glyph {
                        id: font.index('A'),
                        x: (-7).into(),
                        y: (-8).into(),
                    }]
                    .into(),
                }))
                .unwrap(),
            ),
        }]
        .into(),
    };
    let expected = Item::Text {
        font: font.clone(),
        color: 0,
        clip: None,
        glyphs: vec![Glyph {
            id: font.index('A'),
            x: (9).into(),
            y: (10).into(),
        }],
    };
    let frame = |item| {
        Frame::new(
            Scene::new(extent, 0, vec![item]).unwrap(),
            Visual::new(extent),
        )
        .unwrap()
    };
    let expected = Cpu.render(&frame(expected)).unwrap();
    let actual = frame(actual);
    assert_eq!(Cpu.render(&actual).unwrap(), expected);
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    let ticket = queue.submit(target, &actual).unwrap();
    queue.wait(ticket, Duration::from_secs(3)).unwrap();
    compare(&queue.read(ticket).unwrap(), &expected);
    queue.release(ticket).unwrap();
    queue.close(target).unwrap();
}

fn compare(actual: &graphics::shared::Image, expected: &graphics::shared::Image) {
    assert_eq!(actual.extent, expected.extent);
    let mut maximum = 0;
    let mut differences = 0;
    for (actual, expected) in actual.pixels.iter().zip(&expected.pixels) {
        for shift in [0, 8, 16] {
            let delta = ((actual >> shift) & 255).abs_diff((expected >> shift) & 255);
            maximum = maximum.max(delta);
            differences += usize::from(delta > 1);
        }
    }
    assert!(
        maximum <= 1,
        "maximum channel error {maximum}, {differences} channels exceed tolerance"
    );
}

#[test]
fn paths() {
    use graphics::shared::{
        Font, Frame, Ink, Item, Record, Scalar, Scene, Space, Transform, Visual,
    };
    use std::sync::Arc;
    let extent = Extent {
        width: 240,
        height: 200,
    };
    let font = Font {
        face: ::fixtures::face("lato", textengine::Weight::Normal),
        ..Font::default()
    };
    let path = font.outline(font.index('é')).unwrap();
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    let mut previous = None;
    for scale in [0.04, 0.075, 0.12] {
        let angle: f64 = 0.4;
        let transform = Transform(
            [
                scale * angle.cos(),
                scale * angle.sin(),
                scale * angle.sin(),
                -scale * angle.cos(),
                60.375,
                150.625,
            ]
            .map(Scalar::new),
        );
        let record = Record::new(Arc::new(Ink::Path {
            path: path.clone(),
            transform,
            color: 0x18485a,
        }))
        .unwrap();
        let frame = Frame::new(
            Scene::new(
                extent,
                0,
                vec![Item::Group {
                    space: Space {
                        id: 1,
                        translation: [-12, 8],
                        clip: Some(
                            graphics::shared::Rect {
                                x: 0,
                                y: 0,
                                width: 170,
                                height: 180,
                            }
                            .into(),
                        ),
                        scroll: None,
                    },
                    opacity: 137,
                    items: vec![Item::Draw {
                        origin: [0; 2],
                        clip: None,
                        record: Arc::new(record),
                    }]
                    .into(),
                }],
            )
            .unwrap(),
            Visual::new(extent),
        )
        .unwrap();
        let expected = Cpu.render(&frame).unwrap();
        assert!(
            expected
                .pixels
                .iter()
                .filter(|pixel| **pixel != 0xffffff)
                .count()
                > 100
        );
        if let Some(previous) = previous {
            assert_ne!(expected, previous);
        }
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(3)).unwrap();
        compare(&queue.read(ticket).unwrap(), &expected);
        queue.release(ticket).unwrap();
        previous = Some(expected);
    }
    queue.close(target).unwrap();
}

mod inline;

mod pictures;
