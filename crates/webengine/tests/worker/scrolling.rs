use super::*;

pub(super) fn verify(worker: &mut Worker) {
    evaluate(
        worker,
        "let observed = 0; let payload = ''; document.root.addEventListener('scroll', (event) => { observed = observed + 1; payload = event.scroll.offset + ':' + event.scroll.cause + ':' + event.scroll.causes[0] + ':' + event.scroll.causes[1]; bridge.postMessage('scroll'); });",
    );
    let events = worker
        .inspect()
        .events
        .iter()
        .filter(|event| event.kind == "scroll")
        .count();
    worker.scroll(8).unwrap();
    worker.scroll(8).unwrap();
    assert!(worker.messages().is_empty());
    until(worker, |snapshot| {
        snapshot
            .events
            .iter()
            .filter(|event| event.kind == "scroll")
            .count()
            > events
    });
    assert_eq!(worker.messages(), ["scroll"]);
    let id = evaluate(worker, "observed;");
    assert_eq!(
        worker
            .inspect()
            .execution
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .result(),
        Some(&Ok("1".into()))
    );
    let id = evaluate(worker, "payload;");
    assert_eq!(
        worker
            .inspect()
            .execution
            .unwrap()
            .jobs
            .iter()
            .find(|job| job.id == id)
            .unwrap()
            .result(),
        Some(&Ok("64:program:layout:program".into()))
    );
}
