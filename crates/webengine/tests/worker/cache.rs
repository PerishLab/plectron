use super::{Action, Phase, Worker, phase, until};
use shared::cache::Command;

pub(super) fn ordered(worker: &mut Worker) {
    let before = worker.inspect();
    let cache = before.caches[0].id;
    let hold = worker.hold().unwrap();
    until(worker, |snapshot| phase(snapshot, hold) == Phase::Held);
    let job = worker
        .submit(Action::Cache {
            id: cache,
            command: Command::Destroy,
        })
        .unwrap();
    assert_eq!(phase(&worker.inspect(), job), Phase::Queued);
    assert_eq!(worker.inspect().caches, before.caches);
    worker.unhold().unwrap();
    until(worker, |snapshot| phase(snapshot, job) == Phase::Complete);
    let after = worker.inspect();
    let closed = after.caches.iter().find(|entry| entry.id == cache).unwrap();
    assert!(closed.closed);
    assert_eq!(closed.applied, closed.requested);
    assert_eq!(closed.stats.bytes, 0);
    assert_eq!(after.scene, before.scene);
    assert_eq!(after.revision, before.revision);
    let invalid = worker
        .submit(Action::Cache {
            id: cache,
            command: Command::Clear,
        })
        .unwrap();
    until(worker, |snapshot| {
        phase(snapshot, invalid) == Phase::Complete
    });
    assert_eq!(worker.errors(), ["cache is closing"]);
    let committed = worker.inspect().update.committed;
    let failed = super::evaluate(
        worker,
        "document.getElementById('a').setAttribute('style', 'overflow: auto; height: auto;');",
    );
    assert_eq!(phase(&worker.inspect(), failed), Phase::Complete);
    assert_eq!(worker.inspect().update.committed, committed);
    assert!(worker.inspect().update.failure.is_some());
    assert_eq!(worker.errors().len(), 1);
    super::evaluate(
        worker,
        "document.getElementById('a').setAttribute('style', 'height: 32px;');",
    );
    let recovered = worker.inspect();
    assert_eq!(recovered.update.committed, recovered.revision);
    assert_eq!(recovered.update.changes, 2);
    assert_eq!(Some(recovered.painting.revision), recovered.revision);
    assert!(recovered.painting.hits > 0);
    worker.close();
    assert_eq!(worker.inspect().caches, recovered.caches);
}
