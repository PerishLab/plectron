#![cfg(windows)]
use graphics::d3d12::D3d12;
use graphics::shared::{
    Backend, Cpu, Evaluation, Extent, Glyph, Ink, Item, Output, Path, Preparation, Queue, Raster,
    Rasterization, Record, Rect, Scene, Segment, Space, State, Transform,
};
use std::sync::Arc;
use std::time::Duration;

#[test]
fn hardware() {
    let d3d12 = D3d12::new().expect("D3D12 device required");
    eprintln!("D3D12 device: {}", d3d12.name());
    exercise(d3d12);
}

#[test]
fn warp() {
    let d3d12 = D3d12::warp().expect("D3D12 WARP adapter required");
    eprintln!("D3D12 WARP device: {}", d3d12.name());
    exercise(d3d12);
}

fn exercise(d3d12: D3d12) {
    let mut gpu = Queue::new(d3d12);
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
fn warpparity() {
    let mut d3d12 = D3d12::warp().unwrap();
    let extent = Extent {
        width: 96,
        height: 64,
    };
    let raster = Raster::rgba(
        Extent {
            width: 2,
            height: 2,
        },
        &[
            255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 64, 255, 255, 255, 0,
        ],
    )
    .unwrap();
    let path = Path::new(vec![
        Segment::Move([0.into(), 0.into()]),
        Segment::Quad([24.into(), 0.into()], [24.into(), 20.into()]),
        Segment::Quad([0.into(), 20.into()], [0.into(), 0.into()]),
        Segment::Close,
    ])
    .unwrap();
    let frame = record(
        Scene::new(
            extent,
            1,
            vec![
                Item::Fill {
                    rect: Rect {
                        x: 2,
                        y: 2,
                        width: 40,
                        height: 24,
                    },
                    color: Some(0x334455),
                },
                Item::Group {
                    space: Space {
                        id: 1,
                        translation: [6, 4],
                        clip: Some(
                            Rect {
                                x: 4,
                                y: 4,
                                width: 60,
                                height: 48,
                            }
                            .into(),
                        ),
                        scroll: None,
                    },
                    opacity: 160,
                    items: vec![
                        Item::Draw {
                            origin: [10, 8],
                            clip: None,
                            record: Arc::new(
                                Record::new(Arc::new(Ink::Path {
                                    path,
                                    transform: Transform::default(),
                                    color: 0x228844,
                                }))
                                .unwrap(),
                            ),
                        },
                        Item::Draw {
                            origin: [32, 12],
                            clip: None,
                            record: Arc::new(
                                Record::new(Arc::new(Ink::Image {
                                    raster,
                                    rect: Rect {
                                        x: 0,
                                        y: 0,
                                        width: 16,
                                        height: 16,
                                    },
                                }))
                                .unwrap(),
                            ),
                        },
                    ]
                    .into(),
                },
            ],
        )
        .unwrap(),
    );
    for mode in [Rasterization::Direct, Rasterization::Local] {
        let mut preparation = Preparation::default();
        preparation.evaluation(Evaluation::Quadratic);
        preparation.rasterization(mode);
        let pass = preparation
            .prepare(&frame, Output::new(extent, 1.25).unwrap())
            .unwrap();
        let expected = Cpu.submit(&pass).unwrap();
        let work = d3d12.submit(&pass).unwrap();
        d3d12.wait(&work, Duration::from_secs(5)).unwrap();
        let actual = d3d12.read(&work).unwrap();
        assert_eq!(actual.extent, expected.extent);
        let maximum = actual
            .pixels
            .iter()
            .zip(&expected.pixels)
            .flat_map(|(a, b)| {
                [0, 8, 16].map(|shift| ((a >> shift) & 255).abs_diff((b >> shift) & 255))
            })
            .max()
            .unwrap();
        assert!(maximum <= 2, "WARP maximum channel error {maximum}");
    }
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
    let mut gpu = Queue::new(D3d12::new().unwrap());
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
    let mut d3d12 = D3d12::new().unwrap();
    let mut foreign = D3d12::new().unwrap();
    let extent = Extent {
        width: 4,
        height: 4,
    };
    let frame = record(Scene::new(extent, 0, Vec::new()).unwrap());
    let work = d3d12.render(&frame).unwrap();
    assert!(
        foreign
            .wait(&work, Duration::ZERO)
            .unwrap_err()
            .to_string()
            .contains("foreign")
    );
    d3d12.wait(&work, Duration::from_secs(5)).unwrap();
    let retained = work.clone();
    drop(work);
    d3d12.wait(&retained, Duration::ZERO).unwrap();
    assert_eq!(d3d12.read(&retained).unwrap(), Cpu.render(&frame).unwrap());
}
