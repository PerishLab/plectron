#![cfg(target_os = "macos")]
use graphics::metal::{Executor, Metal};
use graphics::shared::{Backend, Cpu, Extent, Glyph, Item, Queue, Rect, Scene, State};
use std::time::Duration;

#[test]
fn hardware() {
    let metal = Metal::new().expect("real Metal device required");
    eprintln!("Metal device: {}", metal.name());
    let mut gpu = Queue::new(metal);
    let extent = Extent {
        width: 67,
        height: 43,
    };
    let target = gpu.target(extent).unwrap();
    let scene = record(
        Scene::new(
            extent,
            1,
            vec![
                Item::Fill {
                    rect: Rect {
                        x: 1,
                        y: 2,
                        width: 80,
                        height: 25,
                    },
                    color: Some(0x123456),
                },
                Item::Text {
                    font: graphics::shared::Font::default(),
                    clip: Some(
                        Rect {
                            x: 4,
                            y: 3,
                            width: 50,
                            height: 31,
                        }
                        .into(),
                    ),
                    glyphs: vec![
                        Glyph {
                            x: 0.into(),
                            y: 0.into(),
                            id: graphics::shared::Font::default().index('A'),
                        },
                        Glyph {
                            x: 16.into(),
                            y: 20.into(),
                            id: graphics::shared::Font::default().index('中'),
                        },
                    ],
                    color: 0xabcdef,
                },
                Item::Fill {
                    rect: Rect {
                        x: 20,
                        y: 5,
                        width: 7,
                        height: 9,
                    },
                    color: Some(0xff00aa),
                },
            ],
        )
        .unwrap(),
    );
    let ticket = gpu.submit(target, &scene).unwrap();
    assert_eq!(gpu.state(ticket).unwrap(), State::Pending);
    assert!(gpu.read(ticket).is_err());
    gpu.wait(ticket, Duration::from_secs(5)).unwrap();
    let actual = gpu.read(ticket).unwrap();
    assert_eq!(actual, Cpu.render(&scene).unwrap());
    let next = gpu
        .submit(target, &record(Scene::new(extent, 2, vec![]).unwrap()))
        .unwrap();
    gpu.close(target).unwrap();
    gpu.wait(next, Duration::from_secs(5)).unwrap();
    assert!(
        gpu.read(next)
            .unwrap()
            .pixels
            .iter()
            .all(|pixel| *pixel == 0xffffff)
    );
    assert_eq!(gpu.read(ticket).unwrap(), actual);
    gpu.release(ticket).unwrap();
    gpu.release(next).unwrap();
    assert_eq!(gpu.retained(), 0);
}

#[test]
fn executor() {
    let extent = Extent {
        width: 31,
        height: 19,
    };
    let frame = record(
        Scene::new(
            extent,
            1,
            vec![Item::Fill {
                rect: Rect {
                    x: 3,
                    y: 5,
                    width: 11,
                    height: 7,
                },
                color: Some(0x123456),
            }],
        )
        .unwrap(),
    );
    let backend = Executor::new(shared::Notify::new(|| {})).unwrap();
    let mut queue = Queue::new(backend);
    let target = queue.target(extent).unwrap();
    let next = record(Scene::new(extent, 2, Vec::new()).unwrap());
    let tickets = [
        queue.submit(target, &frame).unwrap(),
        queue.submit(target, &next).unwrap(),
    ];
    queue.wait(tickets[1], Duration::from_secs(5)).unwrap();
    for (ticket, frame) in tickets.into_iter().zip([frame, next]) {
        assert_eq!(queue.read(ticket).unwrap(), Cpu.render(&frame).unwrap());
        queue.release(ticket).unwrap();
    }
    queue.close(target).unwrap();
}

struct Source;
impl webengine::resource::Provider for Source {
    fn request(
        &mut self,
        _: &webengine::resource::Request,
    ) -> Result<webengine::resource::Response, webengine::Fault> {
        Ok(webengine::resource::Response {
            status: 200,
            mime: "text/html".into(),
            body: b"<body><button id='button'>GPU</button></body>".to_vec(),
        })
    }
}

#[test]
fn webpage() {
    let extent = Extent {
        width: 101,
        height: 73,
    };
    let mut page = webengine::Page::new("app://test/index.html", extent).unwrap();
    page.load(&mut Source).unwrap();
    let old = page.frame().unwrap();
    page.acknowledge(old.id()).unwrap();
    page.evaluate(&fixtures::script(
        "document",
        "document.getElementById('button').setText('Changed');",
    ))
    .unwrap();
    page.render().unwrap();
    let new = page.frame().unwrap();
    page.close();
    let mut gpu = Queue::new(Metal::new().unwrap());
    let target = gpu.target(extent).unwrap();
    let first = gpu.submit(target, &old).unwrap();
    let second = gpu.submit(target, &new).unwrap();
    gpu.wait(second, Duration::from_secs(5)).unwrap();
    let initial = gpu.read(first).unwrap();
    let changed = gpu.read(second).unwrap();
    assert_eq!(initial, Cpu.render(&old).unwrap());
    assert_eq!(changed, Cpu.render(&new).unwrap());
    assert_ne!(initial, changed);
    gpu.release(first).unwrap();
    gpu.release(second).unwrap();
    gpu.close(target).unwrap();
}

fn record(scene: Scene) -> graphics::shared::Frame {
    let visual = graphics::shared::Visual::new(scene.extent());
    graphics::shared::Frame::new(scene, visual).unwrap()
}

#[test]
fn notification() {
    let mut metal = Metal::new().unwrap();
    let mut foreign = Metal::new().unwrap();
    let extent = Extent {
        width: 4,
        height: 4,
    };
    let frame = record(Scene::new(extent, 0, Vec::new()).unwrap());
    let work = metal.render(&frame).unwrap();
    assert!(
        foreign
            .wait(&work, Duration::ZERO)
            .unwrap_err()
            .to_string()
            .contains("foreign")
    );
    metal.wait(&work, Duration::from_secs(5)).unwrap();
    let retained = work.clone();
    drop(work);
    metal.wait(&retained, Duration::ZERO).unwrap();
    assert_eq!(metal.read(&retained).unwrap(), Cpu.render(&frame).unwrap());
}
