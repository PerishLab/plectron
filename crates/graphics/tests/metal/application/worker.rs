#![cfg(target_os = "macos")]
use graphics::metal::Metal;
use graphics::shared::{Backend, Cpu, Extent, Queue};
use std::time::{Duration, Instant};
use webengine::{
    Fault, Phase, State, Worker,
    resource::{Provider, Response},
};

struct Source;
impl Provider for Source {
    fn request(&mut self, _: &webengine::resource::Request) -> Result<Response, Fault> {
        Ok(Response { status:200,mime:"text/html".into(),body:b"<body><button style='height: 32px; background-color: #112233;'>A</button><button style='height: 32px; background-color: #445566;'>B</button></body>".to_vec() })
    }
}
fn until(worker: &mut Worker, predicate: impl Fn(&webengine::Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.poll().unwrap();
        if predicate(&worker.inspect()) {
            return;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn independent() {
    let extent = Extent {
        width: 64,
        height: 16,
    };
    let mut worker = Worker::spawn("app://worker/index.html", extent, Source).unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let held = worker.hold().unwrap();
    until(&mut worker, |snapshot| {
        snapshot
            .execution
            .as_ref()
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == held && job.phase() == Phase::Held)
    });
    let scene = worker.inspect().scene;
    let mut queue = Queue::new(Metal::new().unwrap());
    let target = queue.target(extent).unwrap();
    for delta in [0, 16, 16] {
        worker.scroll(delta).unwrap();
        let frame = worker.frame().unwrap();
        let ticket = queue.submit(target, &frame).unwrap();
        queue.wait(ticket, Duration::from_secs(5)).unwrap();
        assert_eq!(queue.read(ticket).unwrap(), Cpu.render(&frame).unwrap());
        worker.acknowledge(frame.id()).unwrap();
        worker.poll().unwrap();
        let snapshot = worker.inspect();
        assert_eq!(snapshot.scene, scene);
        assert!(
            snapshot
                .execution
                .unwrap()
                .jobs
                .iter()
                .any(|job| job.id == held && job.phase() == Phase::Held)
        );
        queue.release(ticket).unwrap();
    }
    worker.close();
    queue.close(target).unwrap();
}
