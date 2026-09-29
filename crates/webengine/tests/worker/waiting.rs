use super::*;
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
