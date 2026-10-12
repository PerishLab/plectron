use super::*;
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
pub(super) fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}
pub(super) fn ready() -> Worker {
    let mut worker = Worker::spawn(
        "app://worker/index.html",
        Extent {
            width: 64,
            height: 32,
        },
        Source,
    )
    .unwrap();
    until(&mut worker, |snapshot| snapshot.state == State::Ready);
    let frame = worker.frame().unwrap();
    worker.acknowledge(frame.id()).unwrap();
    worker
}
pub(super) fn until(worker: &mut Worker, predicate: impl Fn(&Snapshot) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        worker.poll().unwrap();
        if predicate(&worker.inspect()) {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "worker deadline: {:?}",
            worker.inspect().execution
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

pub(super) fn phase(snapshot: &Snapshot, id: u64) -> Phase {
    snapshot
        .execution
        .as_ref()
        .unwrap()
        .jobs
        .iter()
        .find(|job| job.id == id)
        .unwrap()
        .phase()
}
