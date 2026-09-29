use graphics::shared::{
    Backend, Cpu, Extent, Fault, Glyph, Image, Item, Queue, Rect, Scene, Sink, State, Target,
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn scene() -> graphics::shared::Frame {
    record(
        Scene::new(
            Extent {
                width: 31,
                height: 19,
            },
            7,
            vec![
                Item::Fill {
                    rect: Rect {
                        x: 2,
                        y: 3,
                        width: 40,
                        height: 5,
                    },
                    color: Some(0x123456),
                },
                Item::Text {
                    font: graphics::shared::Font::default(),
                    clip: Some(
                        Rect {
                            x: 0,
                            y: 0,
                            width: 31,
                            height: 19,
                        }
                        .into(),
                    ),
                    glyphs: vec![Glyph {
                        x: 0.into(),
                        y: 0.into(),
                        id: graphics::shared::Font::default().index('A'),
                    }],
                    color: 0xabcdef,
                },
            ],
        )
        .unwrap(),
    )
}

#[test]
fn pixels() {
    let frame = Cpu.render(&scene()).unwrap();
    assert_eq!(frame.pixels.len(), 589);
    assert_eq!(frame.pixels[0], 0xffffff);
    assert_eq!(frame.pixels[3 * 31 + 30], 0x123456);
    assert!(frame.pixels.contains(&0xabcdef));
    let empty = record(Scene::new(frame.extent, 0, vec![]).unwrap());
    assert!(
        Cpu.render(&empty)
            .unwrap()
            .pixels
            .iter()
            .all(|pixel| *pixel == 0xffffff)
    );
}

#[test]
fn ownership() {
    let scene = scene();
    let mut first = Queue::new(Cpu);
    let mut second = Queue::new(Cpu);
    let target = first.target(scene.extent()).unwrap();
    assert!(second.submit(target, &scene).is_err());
    let ticket = first.submit(target, &scene).unwrap();
    assert_eq!(first.state(ticket).unwrap(), State::Pending);
    assert!(first.read(ticket).is_err());
    assert!(second.state(ticket).is_err());
    first.poll().unwrap();
    assert_eq!(first.read(ticket).unwrap(), Cpu.render(&scene).unwrap());
    first.release(ticket).unwrap();
    assert!(first.read(ticket).is_err());
    first.close(target).unwrap();
    assert!(first.submit(target, &scene).is_err());
}

struct Delayed {
    signal: Arc<AtomicUsize>,
    dropped: Arc<AtomicUsize>,
}
struct Work {
    dropped: Arc<AtomicUsize>,
}
impl Drop for Work {
    fn drop(&mut self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
}
impl Backend for Delayed {
    type Work = Work;
    fn submit(&mut self, _: &graphics::shared::Pass) -> Result<Work, Fault> {
        Ok(Work {
            dropped: self.dropped.clone(),
        })
    }
    fn poll(&mut self, _: &mut Work) -> Result<State, Fault> {
        match self.signal.load(Ordering::Relaxed) {
            0 => Ok(State::Pending),
            1 => Ok(State::Complete),
            2 => Ok(State::Failed(Fault("execution failed".into()))),
            _ => Err(Fault("device lost".into())),
        }
    }
    fn read(&mut self, _: &Work) -> Result<Image, Fault> {
        Err(Fault("no pixels".into()))
    }
}

#[test]
fn retention() {
    let signal = Arc::new(AtomicUsize::new(0));
    let dropped = Arc::new(AtomicUsize::new(0));
    let mut queue = Queue::new(Delayed {
        signal: signal.clone(),
        dropped: dropped.clone(),
    });
    let scene = scene();
    let target = queue.target(scene.extent()).unwrap();
    let first = queue.submit(target, &scene).unwrap();
    let second = queue.submit(target, &scene).unwrap();
    assert!(queue.submit(target, &scene).is_err());
    assert!(queue.wait(first, std::time::Duration::ZERO).is_err());
    assert_eq!(queue.state(first).unwrap(), State::Pending);
    queue.close(target).unwrap();
    queue.release(first).unwrap();
    queue.release(second).unwrap();
    queue.poll().unwrap();
    assert_eq!(queue.retained(), 2);
    assert_eq!(dropped.load(Ordering::Relaxed), 0);
    signal.store(1, Ordering::Relaxed);
    queue.poll().unwrap();
    assert_eq!(queue.retained(), 0);
    assert_eq!(dropped.load(Ordering::Relaxed), 2);
}

struct Output {
    target: Target,
    count: usize,
}
impl Sink<Image> for Output {
    fn target(&self) -> Target {
        self.target
    }
    fn present(&mut self, _: &Image) -> Result<(), Fault> {
        self.count += 1;
        Ok(())
    }
}

#[test]
fn presentation() {
    let mut queue = Queue::new(Cpu);
    let scene = scene();
    let target = queue.target(scene.extent()).unwrap();
    let another = queue.target(scene.extent()).unwrap();
    let ticket = queue.submit(target, &scene).unwrap();
    let mut output = Output {
        target: another,
        count: 0,
    };
    assert!(queue.present(ticket, &mut output).is_err());
    queue.poll().unwrap();
    assert!(queue.present(ticket, &mut output).is_err());
    output.target = target;
    queue.present(ticket, &mut output).unwrap();
    assert_eq!(output.count, 1);
    queue.close(target).unwrap();
    assert!(queue.present(ticket, &mut output).is_err());
    assert!(queue.read(ticket).is_ok());
    queue.release(ticket).unwrap();
}

#[test]
fn failures() {
    for mode in [2, 3] {
        let signal = Arc::new(AtomicUsize::new(mode));
        let mut queue = Queue::new(Delayed {
            signal,
            dropped: Arc::new(AtomicUsize::new(0)),
        });
        let scene = scene();
        let target = queue.target(scene.extent()).unwrap();
        let ticket = queue.submit(target, &scene).unwrap();
        assert_eq!(queue.poll().is_err(), mode == 3);
        assert!(matches!(queue.state(ticket).unwrap(), State::Failed(_)));
        assert!(queue.read(ticket).is_err());
        assert_eq!(queue.submit(target, &scene).is_err(), mode == 3);
        queue.release(ticket).unwrap();
    }
}

#[test]
fn limits() {
    let extent = scene().extent();
    assert!(
        Scene::new(
            extent,
            0,
            vec![Item::Fill {
                rect: Rect {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1
                },
                color: Some(0xff000000)
            }]
        )
        .is_err()
    );
    let extreme = record(
        Scene::new(
            extent,
            0,
            vec![Item::Fill {
                rect: Rect {
                    x: usize::MAX,
                    y: usize::MAX,
                    width: usize::MAX,
                    height: usize::MAX,
                },
                color: Some(0),
            }],
        )
        .unwrap(),
    );
    assert!(
        Cpu.render(&extreme)
            .unwrap()
            .pixels
            .iter()
            .all(|pixel| *pixel == 0xffffff)
    );
}

#[test]
fn bitmap() {
    let original = Cpu.render(&scene()).unwrap();
    let translated = record(Scene::bitmap(&original).unwrap());
    assert_eq!(Cpu.render(&translated).unwrap(), original);
    let invalid = Image {
        extent: original.extent,
        pixels: vec![],
    };
    assert!(Scene::bitmap(&invalid).is_err());
}

fn record(scene: Scene) -> graphics::shared::Frame {
    let visual = graphics::shared::Visual::new(scene.extent());
    graphics::shared::Frame::new(scene, visual).unwrap()
}
