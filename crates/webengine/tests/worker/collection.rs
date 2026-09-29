use super::*;

fn collect(worker: &mut Worker) -> (usize, usize) {
    let id = worker.submit(Action::Collect).unwrap();
    until(worker, |snapshot| phase(snapshot, id) == Phase::Complete);
    let snapshot = worker.inspect();
    let execution = snapshot.execution.unwrap();
    let result = execution
        .jobs
        .iter()
        .find(|job| job.id == id)
        .unwrap()
        .result()
        .unwrap()
        .as_ref()
        .unwrap();
    let mut fields = result.split_whitespace();
    let objects = fields
        .next()
        .unwrap()
        .strip_prefix("objects=")
        .unwrap()
        .parse()
        .unwrap();
    let nodes = fields
        .next()
        .unwrap()
        .strip_prefix("nodes=")
        .unwrap()
        .parse()
        .unwrap();
    let fields: std::collections::BTreeMap<_, _> = fields
        .map(|field| field.split_once('=').unwrap())
        .map(|(name, value)| (name, value.parse::<usize>().unwrap()))
        .collect();
    assert_eq!(
        fields["js_live"] + fields["js_vacant"],
        fields["js_capacity"]
    );
    assert_eq!(
        fields["dom_live"] + fields["dom_vacant"],
        fields["dom_capacity"]
    );
    assert!(fields["js_rounds"] >= 1);
    (objects, nodes)
}

pub(super) fn visible(worker: &mut Worker) {
    collect(worker);
    evaluate(
        worker,
        "document.getElementById('body').removeChild(document.getElementById('a'));",
    );
    assert_eq!(collect(worker).1, 0);
    let frame = worker.frame().unwrap();
    worker.acknowledge(frame.id()).unwrap();
    assert!(collect(worker).1 > 0);
}

pub(super) fn pressure(worker: &mut Worker) {
    let source = "document.createElement('div');".repeat(16);
    for _ in 0..264 {
        evaluate(worker, &source);
    }
    assert_eq!(collect(worker).1, 0);
    let source = "({x: 1});".repeat(16);
    for _ in 0..16 {
        evaluate(worker, &source);
    }
    assert_eq!(collect(worker).0, 0);
}
