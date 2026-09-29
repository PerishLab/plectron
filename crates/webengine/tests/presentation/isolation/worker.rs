use super::*;
use std::time::{Duration, Instant};
use webengine::{Phase, State, Worker};

fn until(worker: &mut Worker, condition: impl Fn(&webengine::Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.poll().unwrap();
        assert!(worker.errors().is_empty());
        if condition(&worker.inspect()) {
            return;
        }
        assert!(Instant::now() < deadline, "worker demand timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn coalescing() {
    let mut worker = Worker::spawn(
        "app://test/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        source(true),
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let first = worker.frame().unwrap();
    worker.acknowledge(first.id()).unwrap();
    let hold = worker.hold().unwrap();
    until(&mut worker, |snapshot| {
        snapshot
            .execution
            .as_ref()
            .unwrap()
            .jobs
            .iter()
            .any(|job| job.id == hold && job.phase() == Phase::Held)
    });
    worker.scroll(64).unwrap();
    assert!(worker.frame().is_none());
    worker.scroll(32).unwrap();
    assert!(worker.frame().is_none());
    assert_eq!(worker.inspect().shown.unwrap().offset(0), 0);
    worker.unhold().unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let next = loop {
        worker.poll().unwrap();
        assert!(worker.errors().is_empty());
        if let Some(frame) = worker.frame() {
            break frame;
        }
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(next.visual().offset(0), 96);
    let mut reference = page(false);
    image(&mut reference);
    reference.scroll(96).unwrap();
    assert_eq!(Cpu.render(&next).unwrap(), image(&mut reference));
    worker.acknowledge(next.id()).unwrap();
    worker.close();
}
