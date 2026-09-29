use graphics::shared::Extent;
use std::time::{Duration, Instant};
use webengine::{
    Action, Fault, Phase, State, Worker,
    resource::{Provider, Response},
};

struct Source;
impl Provider for Source {
    fn request(&mut self, _: &webengine::resource::Request) -> Result<Response, Fault> {
        Ok(Response {
            status: 200,
            mime: "text/html".into(),
            body: b"<body>barrier</body>".to_vec(),
        })
    }
}
fn phase(worker: &Worker, id: u64) -> Phase {
    worker
        .inspect()
        .execution
        .unwrap()
        .jobs
        .iter()
        .find(|job| job.id == id)
        .unwrap()
        .phase()
}
fn until(worker: &mut Worker, predicate: impl Fn(&Worker) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        worker.poll().unwrap();
        if predicate(worker) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "{:?}",
            worker.inspect().execution
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}
#[test]
fn generations() {
    let mut worker = Worker::spawn(
        "app://barrier/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        Source,
    )
    .unwrap();
    until(&mut worker, |worker| worker.state() == State::Ready);
    for _ in 0..4 {
        worker.submit(Action::Evaluate("0;".into())).unwrap();
    }
    let first = worker.hold().unwrap();
    worker.unhold().unwrap();
    let middle = worker.submit(Action::Evaluate("42;".into())).unwrap();
    let second = worker.hold().unwrap();
    until(&mut worker, |worker| phase(worker, second) == Phase::Held);
    assert_eq!(phase(&worker, first), Phase::Complete);
    assert_eq!(phase(&worker, middle), Phase::Complete);
    worker.unhold().unwrap();
    until(&mut worker, |worker| {
        phase(worker, second) == Phase::Complete
    });
    worker.close();
}
